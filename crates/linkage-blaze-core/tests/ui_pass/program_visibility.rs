use linkage_blaze_core::{LinkageFixed, LinkageView, linkage_program};

mod declarations {
    use super::*;

    linkage_program! {
        pub(super) Restricted {
            program: LinkageFixed::<0, 0, 1>::start(),
            dof: 0,
            marks: 0,
        }
        #[derive(Debug)]
        pub Public {
            program: LinkageFixed::<0, 0, 1>::start(),
            dof: 0,
            marks: 0,
        }
    }

    const _: LinkageView<'static, 0, 0> = Restricted::VIEW;
    const _: LinkageView<'static, 0, 0> = Public::VIEW;
}

const _: LinkageView<'static, 0, 0> = declarations::Restricted::VIEW;
const _: LinkageView<'static, 0, 0> = declarations::Public::VIEW;

fn main() {}
