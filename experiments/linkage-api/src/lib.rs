#![no_std]
#![forbid(unsafe_code)]

/// One operation in the deliberately small experimental linkage vocabulary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    Start,
    Forward(f32),
    Yaw(f32),
}

/// Fixed linkage storage whose step capacity is erased by [`LinkageView`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkageFixed<const DOF: usize, const MARKS: usize, const STEP_CAPACITY: usize> {
    steps: [Step; STEP_CAPACITY],
    len: usize,
}

impl<const DOF: usize, const MARKS: usize, const STEP_CAPACITY: usize>
    LinkageFixed<DOF, MARKS, STEP_CAPACITY>
{
    /// Begin a fluent linkage with its implicit start step.
    #[must_use]
    pub const fn start() -> Self {
        assert!(
            STEP_CAPACITY > 0,
            "a linkage needs room for its implicit start step"
        );
        Self {
            steps: [Step::Start; STEP_CAPACITY],
            len: 1,
        }
    }

    /// Append a forward movement.
    #[must_use]
    pub const fn forward(mut self, distance: f32) -> Self {
        assert!(self.len < STEP_CAPACITY, "linkage step storage is full");
        self.steps[self.len] = Step::Forward(distance);
        self.len += 1;
        self
    }

    /// Append a yaw rotation.
    #[must_use]
    pub const fn yaw(mut self, degrees: f32) -> Self {
        assert!(self.len < STEP_CAPACITY, "linkage step storage is full");
        self.steps[self.len] = Step::Yaw(degrees);
        self.len += 1;
        self
    }

    /// Borrow the active steps while erasing `STEP_CAPACITY`.
    #[must_use]
    pub const fn view(&self) -> LinkageView<'_, DOF, MARKS> {
        LinkageView {
            steps: self.steps.split_at(self.len).0,
        }
    }
}

/// The operational linkage type; its type does not contain step capacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkageView<'a, const DOF: usize, const MARKS: usize> {
    steps: &'a [Step],
}

impl<'a, const DOF: usize, const MARKS: usize> LinkageView<'a, DOF, MARKS> {
    /// Return the number of runtime parameters.
    #[must_use]
    pub const fn dof(&self) -> usize {
        DOF
    }

    /// Return the number of mark slots.
    #[must_use]
    pub const fn marks(&self) -> usize {
        MARKS
    }

    /// Return the active steps, including the implicit start step.
    #[must_use]
    pub const fn steps(&self) -> &'a [Step] {
        self.steps
    }

    /// Return the number of active steps.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.steps.len()
    }

    /// Return whether this linkage contains no steps.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Materialize two views into exact combined fixed storage.
    ///
    /// This helper is public only because [`linkage_combine!`] may expand in a
    /// downstream crate.
    #[doc(hidden)]
    pub const fn __combine_fixed<
        const OTHER_DOF: usize,
        const OTHER_MARKS: usize,
        const OUTPUT_DOF: usize,
        const OUTPUT_MARKS: usize,
        const OUTPUT_STEP_CAPACITY: usize,
    >(
        self,
        other: LinkageView<'_, OTHER_DOF, OTHER_MARKS>,
    ) -> LinkageFixed<OUTPUT_DOF, OUTPUT_MARKS, OUTPUT_STEP_CAPACITY> {
        assert!(
            OUTPUT_DOF == DOF + OTHER_DOF,
            "combined DOF must equal the sum of input DOF"
        );
        assert!(
            OUTPUT_MARKS == MARKS + OTHER_MARKS,
            "combined MARKS must equal the sum of input MARKS"
        );
        assert!(
            OUTPUT_STEP_CAPACITY == self.len() + other.len() - 1,
            "combined step storage must be exact"
        );

        let mut output = LinkageFixed::start();
        let mut step_index = 1;
        while step_index < self.len() {
            output.steps[output.len] = self.steps[step_index];
            output.len += 1;
            step_index += 1;
        }
        step_index = 1;
        while step_index < other.len() {
            output.steps[output.len] = other.steps[step_index];
            output.len += 1;
            step_index += 1;
        }
        output
    }
}

/// Count fluent operations plus the implicit start step.
///
/// This helper is exported only so [`linkage!`] remains hygienic when used by
/// an integration test or another crate.
#[doc(hidden)]
#[macro_export]
macro_rules! __linkage_api_experiment_step_count {
    () => {
        1usize
    };
    (.$method:ident($($argument:tt)*) $($rest:tt)*) => {
        1usize + $crate::__linkage_api_experiment_step_count!($($rest)*)
    };
}

/// Construct a fluent linkage and expose only its promoted static view.
///
/// The macro counts the fluent calls, selects exact fixed backing storage, and
/// hides that storage and its capacity from the result type.
#[macro_export]
macro_rules! linkage {
    (
        dof: $dof:expr,
        marks: $marks:expr;
        $($chain:tt)*
    ) => {{
        const __VIEW: $crate::LinkageView<'static, { $dof }, { $marks }> =
            $crate::LinkageFixed::<
                { $dof },
                { $marks },
                { $crate::__linkage_api_experiment_step_count!($($chain)*) },
            >::start()
                $($chain)*
                .view();
        __VIEW
    }};
}

/// Combine two or more views and return a promoted view.
#[macro_export]
macro_rules! linkage_combine {
    ($first:expr, $second:expr $(,)?) => {{
        const __COMBINED: $crate::LinkageView<
            'static,
            { ($first).dof() + ($second).dof() },
            { ($first).marks() + ($second).marks() },
        > = ($first)
            .__combine_fixed::<
                _,
                _,
                { ($first).dof() + ($second).dof() },
                { ($first).marks() + ($second).marks() },
                { ($first).len() + ($second).len() - 1 },
            >($second)
            .view();
        __COMBINED
    }};
    ($first:expr, $second:expr, $($rest:expr),+ $(,)?) => {
        $crate::linkage_combine!(
            $crate::linkage_combine!($first, $second),
            $($rest),+
        )
    };
}
