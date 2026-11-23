use std::cell::RefCell;
use windows::core::{implement, HRESULT, PCWSTR, PWSTR, Ref, Result};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    COPYENGINE_E_RECYCLE_FORCE_NUKE, IFileOperationProgressSink, IFileOperationProgressSink_Impl,
    IShellItem, SIGDN_DESKTOPABSOLUTEPARSING, TSF_DELETE_RECYCLE_IF_POSSIBLE,
};

#[derive(Default, Debug, Clone)]
pub struct SinkState {
    pub pre_delete_flags: Vec<u32>,
    pub post_delete_hrs: Vec<HRESULT>,
    pub newly_created_parsing: Vec<Option<String>>,
    pub post_move_hrs: Vec<HRESULT>,
    /// True if `PreDeleteItem` intercepted an attempt by Windows to fall back to permanent deletion (V1/R1/R2).
    pub blocked_permanent_delete: bool,
}

#[implement(IFileOperationProgressSink)]
pub struct RecycleProgressSink {
    pub state: RefCell<SinkState>,
}

impl RecycleProgressSink {
    pub fn new() -> Self {
        Self {
            state: RefCell::new(SinkState::default()),
        }
    }
}

fn item_parsing_name(item: &IShellItem) -> Option<String> {
    unsafe {
        let pwstr: PWSTR = item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING).ok()?;
        let s = pwstr.to_string().ok();
        CoTaskMemFree(Some(pwstr.0 as *const _));
        s
    }
}

impl IFileOperationProgressSink_Impl for RecycleProgressSink_Impl {
    fn StartOperations(&self) -> Result<()> {
        Ok(())
    }

    fn FinishOperations(&self, _hrresult: HRESULT) -> Result<()> {
        Ok(())
    }

    fn PreRenameItem(&self, _: u32, _: Ref<'_, IShellItem>, _: &PCWSTR) -> Result<()> {
        Ok(())
    }

    fn PostRenameItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<'_, IShellItem>,
    ) -> Result<()> {
        Ok(())
    }

    fn PreMoveItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
    ) -> Result<()> {
        Ok(())
    }

    fn PostMoveItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
        hrmove: HRESULT,
        _: Ref<'_, IShellItem>,
    ) -> Result<()> {
        self.state.borrow_mut().post_move_hrs.push(hrmove);
        Ok(())
    }

    fn PreCopyItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
    ) -> Result<()> {
        Ok(())
    }

    fn PostCopyItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<'_, IShellItem>,
    ) -> Result<()> {
        Ok(())
    }

    fn PreDeleteItem(&self, dwflags: u32, _psiitem: Ref<'_, IShellItem>) -> Result<()> {
        let mut st = self.state.borrow_mut();
        st.pre_delete_flags.push(dwflags);
        // Critical R1/R2/V1 barrier: if Windows omits TSF_DELETE_RECYCLE_IF_POSSIBLE,
        // it is attempting to permanently delete. Abort immediately before touching disk.
        if (dwflags & (TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32)) == 0 {
            st.blocked_permanent_delete = true;
            return Err(COPYENGINE_E_RECYCLE_FORCE_NUKE.into());
        }
        Ok(())
    }

    fn PostDeleteItem(
        &self,
        _dwflags: u32,
        _psiitem: Ref<'_, IShellItem>,
        hrdelete: HRESULT,
        psinewlycreated: Ref<'_, IShellItem>,
    ) -> Result<()> {
        let mut st = self.state.borrow_mut();
        st.post_delete_hrs.push(hrdelete);
        st.newly_created_parsing
            .push(psinewlycreated.as_ref().and_then(item_parsing_name));
        Ok(())
    }

    fn PreNewItem(&self, _: u32, _: Ref<'_, IShellItem>, _: &PCWSTR) -> Result<()> {
        Ok(())
    }

    fn PostNewItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
        _: &PCWSTR,
        _: u32,
        _: HRESULT,
        _: Ref<'_, IShellItem>,
    ) -> Result<()> {
        Ok(())
    }

    fn UpdateProgress(&self, _: u32, _: u32) -> Result<()> {
        Ok(())
    }

    fn ResetTimer(&self) -> Result<()> {
        Ok(())
    }

    fn PauseTimer(&self) -> Result<()> {
        Ok(())
    }

    fn ResumeTimer(&self) -> Result<()> {
        Ok(())
    }
}
