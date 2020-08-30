/// run some infallible tasks in init for the purpose of side effects
/// e.g. commit some cap grants and move on
#[macro_export]
macro_rules! simple_init {
    ( $do_stuff:expr ) => {
        #[hdk_extern]
        fn init(_: ()) -> $crate::prelude::ExternResult<$crate::prelude::InitCallbackResult> {
            $expr
            Ok($crate::prelude::InitCallbackResult::Success)
        }
    }
}
