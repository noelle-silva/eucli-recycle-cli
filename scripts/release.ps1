<#
.SYNOPSIS
    构建、打包并发布 eucli-recycle 到 GitHub Releases。

.DESCRIPTION
    1. 读取环境变量 EUCLI_RECYCLE_RELEASE_TOKEN（优先当前会话，其次 User 级永久变量）
    2. 校验工作区干净、版本号一致且对应 tag 尚未存在
    3. 执行 cargo build --release 与 cargo test（可用 -SkipTests 跳过测试）
    4. 在系统临时目录打包 zip、独立 exe 与 SHA256SUMS.txt（不污染仓库目录）
    5. 打 git tag、推送 tag，并通过 GitHub REST API 创建 Release 并上传全部附件

.EXAMPLE
    .\scripts\release.ps1 -DryRun
    .\scripts\release.ps1
    .\scripts\release.ps1 -Version 0.1.0 -Draft
#>
[CmdletBinding()]
param(
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

try {
    # 1. Read version from Cargo.toml
    $cargoToml = Get-Content -Path (Join-Path $RepoRoot "Cargo.toml") -Raw -Encoding UTF8
    if ($cargoToml -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
        throw "无法从 Cargo.toml 解析 version 字段。"
    }
    $cargoVersion = $Matches[1]
    if ([string]::IsNullOrWhiteSpace($Version)) {
        $Version = $cargoVersion
    } elseif ($Version -ne $cargoVersion) {
        throw "传入的 -Version ($Version) 与 Cargo.toml 中的版本 ($cargoVersion) 不一致，请先同步 Cargo.toml。"
    }

    $Tag = "v$Version"
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
        throw "本地已存在标签 $Tag，请更新版本号或确认不要重复发布。"
    }

    # 6. Build release binary
    Write-Host "==> 执行 cargo build --release ..."
    & cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build --release 失败（退出码 $LASTEXITCODE）。"
    }

    # 7. Run test suite unless skipped
    if (-not $SkipTests) {
        Write-Host "==> 执行 cargo test ..."
        & cargo test
        if ($LASTEXITCODE -ne 0) {
            throw "cargo test 失败（退出码 $LASTEXITCODE），已中止发布。"
        }
    } else {
        Write-Host "==> 已指定 -SkipTests，跳过测试步骤"
    }

    # 8. Verify built executable
    $ExePath = Join-Path $RepoRoot "target\release\eucli-recycle.exe"
    if (-not (Test-Path $ExePath)) {
        throw "找不到发布产物：$ExePath"
    }
    $verOut = (& $ExePath --version).Trim()
    if ($LASTEXITCODE -ne 0 -or ($verOut -notlike "*$Version*")) {
        throw "可执行文件版本校验失败：$verOut（期望包含 $Version）"
    }
    Write-Host "==> 版本校验通过: $verOut"

    # 9. Package inside isolated temp directory
    $StageDir = Join-Path $env:TEMP ("eucli_recycle_release_" + $Version + "_" + $PID)
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

    $zipHash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $exeHash = (Get-FileHash -Path $ExeCopyPath -Algorithm SHA256).Hash.ToLowerInvariant()

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
        Write-Host "==> [DryRun] 检查与打包全部通过，未创建 tag、未调用 GitHub 写入接口。"
        return
    }

    # 10. Create and push annotated git tag
    Write-Host "==> 创建并推送标签 $Tag ..."
    & git tag -a $Tag -m "Release $Tag"
    if ($LASTEXITCODE -ne 0) {
        throw "创建 git tag 失败。"
    }
    & git push origin $Tag
    if ($LASTEXITCODE -ne 0) {
        throw "推送 git tag 到 origin 失败。"
    }

    # 11. Create GitHub Release via REST API
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

    # 12. Upload release assets
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
finally {
    if ($StageDir -and (Test-Path $StageDir)) {
        Remove-Item -Path $StageDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    Pop-Location
}
