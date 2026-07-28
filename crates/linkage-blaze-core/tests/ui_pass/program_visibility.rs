use linkage_blaze_core::{LinkageFixed, linkage_program};

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

    const _: LinkageFixed<0, 0, 1> = Restricted::fixed();
    const _: LinkageFixed<0, 0, 1> = Public::fixed();
}

const _: LinkageFixed<0, 0, 1> = declarations::Restricted::fixed();
const _: LinkageFixed<0, 0, 1> = declarations::Public::fixed();

fn main() {}
