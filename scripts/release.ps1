<#
.SYNOPSIS
    构建、可选自动递增版本、打包并发布 eucli-recycle 到 GitHub Releases。

.DESCRIPTION
    1. 读取环境变量 EUCLI_RECYCLE_RELEASE_TOKEN（优先当前会话，其次 User 级永久变量）
    2. 校验工作区干净；如指定 -Bump (patch|minor|major)，自动递增 Cargo.toml 版本
    3. 校验目标 tag 尚未存在，执行 cargo build --release 与 cargo test（可用 -SkipTests 跳过）
    4. 在系统临时目录打包 zip、独立 exe 与 SHA256SUMS.txt（不污染仓库目录）
    5. 如发生版本递增，自动提交 Cargo.toml/Cargo.lock 并推送 main；随后打 tag、推送 tag、创建 GitHub Release 并上传附件

.EXAMPLE
    .\scripts\release.ps1 -DryRun
    .\scripts\release.ps1 -Bump patch -DryRun
    .\scripts\release.ps1
    .\scripts\release.ps1 -Bump patch
    .\scripts\release.ps1 -Bump minor -Draft
#>
[CmdletBinding()]
param(
    [ValidateSet('', 'patch', 'minor', 'major')]
    [string]$Bump = "",
    [string]$Version = "",
    [string]$Notes = "",
    [switch]$Draft,
    [switch]$Prerelease,
    [switch]$SkipTests,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $RepoRoot

$StageDir = $null
$CargoModified = $false
$CommittedBump = $false

try {
    if (-not [string]::IsNullOrWhiteSpace($Bump) -and -not [string]::IsNullOrWhiteSpace($Version)) {
        throw "-Bump 与 -Version 不能同时指定，请二选一。"
    }

    # 1. Read current version from Cargo.toml
    $cargoTomlPath = Join-Path $RepoRoot "Cargo.toml"
    $cargoToml = Get-Content -Path $cargoTomlPath -Raw -Encoding UTF8
    if ($cargoToml -notmatch '(?m)^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"') {
        throw "无法从 Cargo.toml 解析 semver 格式的 version 字段（期望 X.Y.Z）。"
    }
    $maj = [int]$Matches[1]
    $min = [int]$Matches[2]
    $pat = [int]$Matches[3]
    $cargoVersion = "$maj.$min.$pat"

    $TargetVersion = $cargoVersion
    $NeedBumpWrite = $false

    if (-not [string]::IsNullOrWhiteSpace($Bump)) {
        switch ($Bump) {
            'patch' { $TargetVersion = "$maj.$min.$($pat + 1)" }
            'minor' { $TargetVersion = "$maj.$($min + 1).0" }
            'major' { $TargetVersion = "$($maj + 1).0.0" }
        }
        $NeedBumpWrite = $true
        Write-Host "==> 版本递增 ($Bump): $cargoVersion -> $TargetVersion"
    } elseif (-not [string]::IsNullOrWhiteSpace($Version)) {
        if ($Version -ne $cargoVersion) {
            if ($Version -notmatch '^\d+\.\d+\.\d+$') {
                throw "非法版本号格式：$Version（期望 X.Y.Z）"
            }
            $TargetVersion = $Version
            $NeedBumpWrite = $true
            Write-Host "==> 指定版本更新: $cargoVersion -> $TargetVersion"
        }
    }

    $Tag = "v$TargetVersion"
    Write-Host "==> 准备发布 eucli-recycle $Tag"

    # 2. Read Fine-grained PAT (Process first, then User)
    $Token = [Environment]::GetEnvironmentVariable('EUCLI_RECYCLE_RELEASE_TOKEN', 'Process')
    if ([string]::IsNullOrWhiteSpace($Token)) {
        $Token = [Environment]::GetEnvironmentVariable('EUCLI_RECYCLE_RELEASE_TOKEN', 'User')
    }
    if ([string]::IsNullOrWhiteSpace($Token)) {
        throw "未找到环境变量 EUCLI_RECYCLE_RELEASE_TOKEN，请先配置 GitHub Fine-grained PAT。"
    }
    Write-Host "==> 已读取发布令牌（长度: $($Token.Length)）"

    # 3. Parse owner/repo from git remote origin
    $originUrl = (& git remote get-url origin).Trim()
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($originUrl)) {
        throw "无法获取 git origin 远程地址。"
    }
    if ($originUrl -match 'github\.com[:/]([^/]+)/([^/\.]+?)(\.git)?$') {
        $Owner = $Matches[1]
        $Repo = $Matches[2]
    } else {
        throw "无法从 origin 地址解析 GitHub 仓库：$originUrl"
    }
    Write-Host "==> 目标仓库: $Owner/$Repo"

    # 4. Verify git working tree is clean (unless -DryRun)
    $dirty = & git status --porcelain
    if (-not $DryRun -and -not [string]::IsNullOrWhiteSpace(($dirty -join ""))) {
        throw "工作区存在未提交改动，请先提交后再发布：`n$($dirty -join "`n")"
    }

    # 5. Check tag does not already exist locally
    $localTag = & git tag -l $Tag
    if (-not [string]::IsNullOrWhiteSpace(($localTag -join ""))) {
        throw "本地已存在标签 $Tag，请使用 -Bump patch|minor|major 递增版本号。"
    }

    # 6. Apply version bump to Cargo.toml (skipped in -DryRun)
    if ($NeedBumpWrite) {
        if ($DryRun) {
            Write-Host "==> [DryRun] 将把 Cargo.toml 版本从 $cargoVersion 更新为 $TargetVersion（DryRun 下不改文件）"
        } else {
            $regex = New-Object System.Text.RegularExpressions.Regex('(?m)^version\s*=\s*"[^"]+"')
            $updatedToml = $regex.Replace($cargoToml, "version = `"$TargetVersion`"", 1)
            [System.IO.File]::WriteAllText($cargoTomlPath, $updatedToml, (New-Object System.Text.UTF8Encoding($false)))
            $CargoModified = $true
            Write-Host "==> 已更新 Cargo.toml 版本为 $TargetVersion"
        }
    }

    # 7. Build release binary (also refreshes Cargo.lock if version bumped)
    Write-Host "==> 执行 cargo build --release ..."
    & cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build --release 失败（退出码 $LASTEXITCODE）。"
    }

    # 8. Run test suite unless skipped
    if (-not $SkipTests) {
        Write-Host "==> 执行 cargo test ..."
        & cargo test
        if ($LASTEXITCODE -ne 0) {
            throw "cargo test 失败（退出码 $LASTEXITCODE），已中止发布。"
        }
    } else {
        Write-Host "==> 已指定 -SkipTests，跳过测试步骤"
    }

    # 9. Verify built executable
    $ExePath = Join-Path $RepoRoot "target\release\eucli-recycle.exe"
    if (-not (Test-Path $ExePath)) {
        throw "找不到发布产物：$ExePath"
    }
    $verOut = (& $ExePath --version).Trim()
    $expectedBuiltVersion = if ($DryRun -and $NeedBumpWrite) { $cargoVersion } else { $TargetVersion }
    if ($LASTEXITCODE -ne 0 -or ($verOut -notlike "*$expectedBuiltVersion*")) {
        throw "可执行文件版本校验失败：$verOut（期望包含 $expectedBuiltVersion）"
    }
    Write-Host "==> 版本校验通过: $verOut"

    # 10. Package inside isolated temp directory
    $StageDir = Join-Path $env:TEMP ("eucli_recycle_release_" + $TargetVersion + "_" + $PID)
    $BundleDir = Join-Path $StageDir ("eucli-recycle-$Tag-windows-x86_64")
    New-Item -ItemType Directory -Force -Path $BundleDir | Out-Null

    Copy-Item -Path $ExePath -Destination (Join-Path $BundleDir "eucli-recycle.exe")
    Copy-Item -Path (Join-Path $RepoRoot "README.md") -Destination (Join-Path $BundleDir "README.md")
    Copy-Item -Path (Join-Path $RepoRoot "SPEC.md") -Destination (Join-Path $BundleDir "SPEC.md")

    $ZipName = "eucli-recycle-$Tag-windows-x86_64.zip"
    $ZipPath = Join-Path $StageDir $ZipName
    Compress-Archive -Path (Join-Path $BundleDir "*") -DestinationPath $ZipPath -Force

    $ExeAssetName = "eucli-recycle.exe"
    $ExeCopyPath = Join-Path $StageDir $ExeAssetName
    Copy-Item -Path $ExePath -Destination $ExeCopyPath

    function Get-Sha256Hex([string]$FilePath) {
        $sha = [System.Security.Cryptography.SHA256]::Create()
        try {
            $fs = [System.IO.File]::OpenRead($FilePath)
            try {
                $hashBytes = $sha.ComputeHash($fs)
                return ([System.BitConverter]::ToString($hashBytes) -replace '-', '').ToLowerInvariant()
            } finally {
                $fs.Dispose()
            }
        } finally {
            $sha.Dispose()
        }
    }

    $zipHash = Get-Sha256Hex $ZipPath
    $exeHash = Get-Sha256Hex $ExeCopyPath

    $SumsName = "SHA256SUMS.txt"
    $SumsPath = Join-Path $StageDir $SumsName
    $sumsContent = "$zipHash  $ZipName`n$exeHash  $ExeAssetName`n"
    [System.IO.File]::WriteAllText($SumsPath, $sumsContent, (New-Object System.Text.UTF8Encoding($false)))

    Write-Host "==> 打包完成:"
    Write-Host "    - $ZipName (SHA256: $zipHash)"
    Write-Host "    - $ExeAssetName (SHA256: $exeHash)"
    Write-Host "    - $SumsName"

    if ([string]::IsNullOrWhiteSpace($Notes)) {
        $Notes = @"
## eucli-recycle $Tag (Windows x64)

### 产物清单
- ``$ZipName``（含 ``eucli-recycle.exe``、``README.md``、``SPEC.md``）
- ``$ExeAssetName``（独立可执行文件）
- ``$SumsName``（SHA-256 校验文件）

### SHA-256 校验值
``````text
$zipHash  $ZipName
$exeHash  $ExeAssetName
``````
"@
    }

    if ($DryRun) {
        Write-Host ""
        Write-Host "==> [DryRun] 检查与打包全部通过，未改文件、未提交、未创建 tag、未调用 GitHub 写入接口。"
        return
    }

    # 11. Commit version bump (if any) and push current branch
    $currentBranch = (& git rev-parse --abbrev-ref HEAD).Trim()
    if ($CargoModified) {
        Write-Host "==> 提交版本更新 commit (chore: release $Tag) ..."
        & git add Cargo.toml Cargo.lock
        if ($LASTEXITCODE -ne 0) { throw "git add 失败。" }
        & git commit -m "chore: release $Tag"
        if ($LASTEXITCODE -ne 0) { throw "git commit 失败。" }
        $CommittedBump = $true
    }

    Write-Host "==> 推送分支 $currentBranch 到 origin ..."
    & git push origin $currentBranch
    if ($LASTEXITCODE -ne 0) {
        throw "推送分支 $currentBranch 失败。"
    }

    # 12. Create and push annotated git tag
    Write-Host "==> 创建并推送标签 $Tag ..."
    & git tag -a $Tag -m "Release $Tag"
    if ($LASTEXITCODE -ne 0) {
        throw "创建 git tag 失败。"
    }
    & git push origin $Tag
    if ($LASTEXITCODE -ne 0) {
        throw "推送 git tag 到 origin 失败。"
    }

    # 13. Create GitHub Release via REST API
    $headers = @{
        "Authorization"        = "Bearer $Token"
        "Accept"               = "application/vnd.github+json"
        "X-GitHub-Api-Version" = "2022-11-28"
        "User-Agent"           = "eucli-recycle-release-script"
    }

    $releasePayload = @{
        tag_name   = $Tag
        name       = "eucli-recycle $Tag"
        body       = $Notes
        draft      = [bool]$Draft
        prerelease = [bool]$Prerelease
    } | ConvertTo-Json -Depth 5

    $releaseBytes = [System.Text.Encoding]::UTF8.GetBytes($releasePayload)
    $createUri = "https://api.github.com/repos/$Owner/$Repo/releases"

    Write-Host "==> 创建 GitHub Release ..."
    $release = Invoke-RestMethod `
        -Method Post `
        -Uri $createUri `
        -Headers $headers `
        -ContentType "application/json; charset=utf-8" `
        -Body $releaseBytes

    $uploadBase = ($release.upload_url -replace '\{\?name,label\}$', '')

    # 14. Upload release assets
    $assets = @(
        @{ Path = $ZipPath;     Name = $ZipName;      ContentType = "application/zip" },
        @{ Path = $ExeCopyPath; Name = $ExeAssetName; ContentType = "application/octet-stream" },
        @{ Path = $SumsPath;    Name = $SumsName;     ContentType = "text/plain; charset=utf-8" }
    )

    foreach ($asset in $assets) {
        $uploadUri = "{0}?name={1}" -f $uploadBase, [Uri]::EscapeDataString($asset.Name)
        Write-Host "==> 上传附件: $($asset.Name) ..."
        $null = Invoke-RestMethod `
            -Method Post `
            -Uri $uploadUri `
            -Headers $headers `
            -ContentType $asset.ContentType `
            -InFile $asset.Path
    }

    Write-Host ""
    Write-Host "==> 发布成功！"
    Write-Host "    Release 页面: $($release.html_url)"
}
catch {
    # If Cargo.toml/Cargo.lock were edited before commit failed, restore cleanly
    if ($CargoModified -and -not $CommittedBump) {
        & git checkout -- Cargo.toml Cargo.lock 2>$null
        Write-Host "==> 已回滚未提交的 Cargo.toml / Cargo.lock 版本改动。"
    }
    throw
}
finally {
    if ($StageDir -and (Test-Path $StageDir)) {
        Remove-Item -Path $StageDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    Pop-Location
}
