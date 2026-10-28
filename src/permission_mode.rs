//! Provider-neutral permission-mode cycle derived from the original UI mode
//! state machine. The original product's special user-type branch is removed.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Mode {
    Default,AcceptEdits,Plan,BypassPermissions,DontAsk,Auto,
}
impl Mode {
    pub fn label(self)->&'static str {
        match self {
            Self::Default=>"default",
            Self::AcceptEdits=>"acceptEdits",
            Self::Plan=>"plan",
            Self::BypassPermissions=>"bypassPermissions",
            Self::DontAsk=>"dontAsk",
            Self::Auto=>"auto",
        }
    }
    pub fn parse(value:&str)->Option<Self>{
        Some(match value {
            "default"=>Self::Default,
            "acceptEdits"=>Self::AcceptEdits,
            "plan"=>Self::Plan,
            "bypassPermissions"=>Self::BypassPermissions,
            "dontAsk"=>Self::DontAsk,
            "auto"=>Self::Auto,
            _=>return None,
        })
    }
}
#[derive(Clone,Copy,Debug,Default)]
pub struct Availability{
    pub bypass:bool,pub auto_cached:bool,pub auto_gate_live:bool
}
pub fn next(mode:Mode,available:Availability)->Mode {
    let auto=available.auto_cached&&available.auto_gate_live;
    match mode {
        Mode::Default=>Mode::AcceptEdits,
        Mode::AcceptEdits=>Mode::Plan,
        Mode::Plan=>{
            if available.bypass {Mode::BypassPermissions}
            else if auto {Mode::Auto}
            else {Mode::Default}
        }
        Mode::BypassPermissions=>{
            if auto {Mode::Auto}else{Mode::Default}
        }
        Mode::DontAsk|Mode::Auto=>Mode::Default,
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn original_nonbrand_permission_mode_cycle_matrix(){
        let no=Availability::default();
        assert_eq!(next(Mode::Default,no),Mode::AcceptEdits);
        assert_eq!(next(Mode::AcceptEdits,no),Mode::Plan);
        assert_eq!(next(Mode::Plan,no),Mode::Default);
        assert_eq!(next(Mode::DontAsk,no),Mode::Default);
        assert_eq!(next(Mode::BypassPermissions,no),Mode::Default);
        let bypass=Availability{bypass:true,..no};
        assert_eq!(next(Mode::Plan,bypass),Mode::BypassPermissions);
        assert_eq!(next(Mode::BypassPermissions,bypass),Mode::Default);
        let cached=Availability{auto_cached:true,..no};
        assert_eq!(next(Mode::Plan,cached),Mode::Default);
        let live=Availability{auto_gate_live:true,..no};
        assert_eq!(next(Mode::Plan,live),Mode::Default);
        let auto=Availability{auto_cached:true,auto_gate_live:true,..no};
        assert_eq!(next(Mode::Plan,auto),Mode::Auto);
        assert_eq!(next(Mode::BypassPermissions,auto),Mode::Auto);
        assert_eq!(next(Mode::Auto,auto),Mode::Default);
        assert_eq!(next(Mode::Plan,Availability{bypass:true,..auto}),Mode::BypassPermissions);
    }
    #[test]
    fn reversible_external_mode_labels(){
        for mode in [
            Mode::Default,Mode::AcceptEdits,Mode::Plan,
            Mode::BypassPermissions,Mode::DontAsk,Mode::Auto
        ]{
            assert_eq!(Mode::parse(mode.label()),Some(mode));
        }
        assert_eq!(Mode::parse("invalid"),None);
    }
}
