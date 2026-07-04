use super::{
    CALIBRATION_POINT_COUNT, CalibrationCorner, RawPoint, RawTouchEvent,
    calibration_corner_for_index,
};

const SAMPLES_DISCARDED_AFTER_DOWN: usize = 1;
const SAMPLE_CAPACITY: usize = 16;
const MIN_SAMPLES_PER_POINT: usize = 1;

/// Sans-io state machine for the four-tap calibration flow.
///
/// Callers own I/O: they draw [`CalibrationCorner`] crosses, log progress, and
/// persist the finished solve. This flow tracks which corner is next and
/// accumulates per-touch raw samples until all four corners have been captured
/// on release.
pub struct CalibrationFlow {
    calibration_index: usize,
    calibration_points: [RawPoint; CALIBRATION_POINT_COUNT],
    release_touch_capture: ReleaseTouchCapture,
}

#[derive(Clone, Copy)]
enum ReleaseTouchCaptureState {
    Armed,
    Sampling {
        discarded_sample_count: usize,
        usable_sample_count: usize,
        usable_samples: [RawPoint; SAMPLE_CAPACITY],
    },
    WaitForIdle,
}

pub(super) struct ReleaseTouchCapture {
    release_touch_capture_state: ReleaseTouchCaptureState,
}

impl CalibrationFlow {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            calibration_index: 0,
            calibration_points: [RawPoint { x: 0, y: 0 }; CALIBRATION_POINT_COUNT],
            release_touch_capture: ReleaseTouchCapture::new(),
        }
    }

    pub fn restart(&mut self) {
        self.calibration_index = 0;
        self.calibration_points = [RawPoint { x: 0, y: 0 }; CALIBRATION_POINT_COUNT];
        self.release_touch_capture.restart();
    }

    #[must_use]
    pub fn next_corner(&self) -> Option<CalibrationCorner> {
        calibration_corner_for_index(self.calibration_index)
    }

    #[must_use]
    pub fn calibration_index(&self) -> usize {
        self.calibration_index
    }

    pub fn handle_raw_touch_event(
        &mut self,
        raw_touch_event: Option<RawTouchEvent>,
    ) -> Option<CalibrationFlowEvent> {
        let Some(calibration_corner) = self.next_corner() else {
            return None;
        };

        let Some(release_touch_capture_event) = self
            .release_touch_capture
            .handle_raw_touch_event(raw_touch_event)
        else {
            return None;
        };

        let ReleaseTouchCaptureEvent::Captured {
            raw_point,
            usable_sample_count,
        } = release_touch_capture_event;
        self.calibration_points[self.calibration_index] = raw_point;
        self.calibration_index += 1;

        if self.calibration_index == CALIBRATION_POINT_COUNT {
            return Some(CalibrationFlowEvent::Completed {
                raw_points: self.calibration_points,
                calibration_corner,
                usable_sample_count,
            });
        }

        Some(CalibrationFlowEvent::PointCaptured {
            calibration_corner,
            raw_point,
            next_corner: self
                .next_corner()
                .expect("next corner exists until calibration completes"),
            usable_sample_count,
        })
    }
}

impl ReleaseTouchCapture {
    pub const fn new() -> Self {
        Self {
            release_touch_capture_state: ReleaseTouchCaptureState::Armed,
        }
    }

    pub fn restart(&mut self) {
        self.release_touch_capture_state = ReleaseTouchCaptureState::Armed;
    }

    pub fn handle_raw_touch_event(
        &mut self,
        raw_touch_event: Option<RawTouchEvent>,
    ) -> Option<ReleaseTouchCaptureEvent> {
        match self.release_touch_capture_state {
            ReleaseTouchCaptureState::Armed => {
                let Some(RawTouchEvent::Down { raw_x, raw_y }) = raw_touch_event else {
                    return None;
                };
                let mut usable_samples = [RawPoint { x: 0, y: 0 }; SAMPLE_CAPACITY];
                usable_samples[0] = RawPoint { x: raw_x, y: raw_y };
                self.release_touch_capture_state = ReleaseTouchCaptureState::Sampling {
                    discarded_sample_count: 0,
                    usable_sample_count: 1,
                    usable_samples,
                };
                None
            }
            ReleaseTouchCaptureState::Sampling {
                discarded_sample_count,
                usable_sample_count,
                mut usable_samples,
            } => match raw_touch_event {
                Some(RawTouchEvent::Down { raw_x, raw_y })
                | Some(RawTouchEvent::Move { raw_x, raw_y }) => {
                    if discarded_sample_count < SAMPLES_DISCARDED_AFTER_DOWN {
                        self.release_touch_capture_state = ReleaseTouchCaptureState::Sampling {
                            discarded_sample_count: discarded_sample_count + 1,
                            usable_sample_count,
                            usable_samples,
                        };
                        return None;
                    }

                    let usable_sample_count = store_usable_sample(
                        &mut usable_samples,
                        usable_sample_count,
                        RawPoint { x: raw_x, y: raw_y },
                    );
                    self.release_touch_capture_state = ReleaseTouchCaptureState::Sampling {
                        discarded_sample_count,
                        usable_sample_count,
                        usable_samples,
                    };
                    None
                }
                Some(RawTouchEvent::Up) => {
                    self.release_touch_capture_state = ReleaseTouchCaptureState::WaitForIdle;
                    if usable_sample_count < MIN_SAMPLES_PER_POINT {
                        return None;
                    }

                    Some(ReleaseTouchCaptureEvent::Captured {
                        raw_point: average_samples(&usable_samples, usable_sample_count),
                        usable_sample_count,
                    })
                }
                None => None,
            },
            ReleaseTouchCaptureState::WaitForIdle => {
                if raw_touch_event.is_none() {
                    self.release_touch_capture_state = ReleaseTouchCaptureState::Armed;
                }
                None
            }
        }
    }
}

impl Default for CalibrationFlow {
    fn default() -> Self {
        Self::new()
    }
}

pub enum CalibrationFlowEvent {
    PointCaptured {
        calibration_corner: CalibrationCorner,
        raw_point: RawPoint,
        next_corner: CalibrationCorner,
        usable_sample_count: usize,
    },
    Completed {
        raw_points: [RawPoint; CALIBRATION_POINT_COUNT],
        calibration_corner: CalibrationCorner,
        usable_sample_count: usize,
    },
}

pub(super) enum ReleaseTouchCaptureEvent {
    Captured {
        raw_point: RawPoint,
        usable_sample_count: usize,
    },
}

fn store_usable_sample(
    usable_samples: &mut [RawPoint; SAMPLE_CAPACITY],
    usable_sample_count: usize,
    raw_point: RawPoint,
) -> usize {
    if usable_sample_count < SAMPLE_CAPACITY {
        usable_samples[usable_sample_count] = raw_point;
        return usable_sample_count + 1;
    }

    let mut sample_index = 1;
    while sample_index < SAMPLE_CAPACITY {
        usable_samples[sample_index - 1] = usable_samples[sample_index];
        sample_index += 1;
    }
    usable_samples[SAMPLE_CAPACITY - 1] = raw_point;
    SAMPLE_CAPACITY
}

fn average_samples(
    usable_samples: &[RawPoint; SAMPLE_CAPACITY],
    usable_sample_count: usize,
) -> RawPoint {
    let mut sum_x = 0_u32;
    let mut sum_y = 0_u32;
    let mut sample_index = 0;
    while sample_index < usable_sample_count {
        let raw_point = usable_samples[sample_index];
        sum_x += u32::from(raw_point.x);
        sum_y += u32::from(raw_point.y);
        sample_index += 1;
    }

    let usable_sample_count = usable_sample_count as u32;
    RawPoint {
        x: ((sum_x + usable_sample_count / 2) / usable_sample_count) as u16,
        y: ((sum_y + usable_sample_count / 2) / usable_sample_count) as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CalibrationFlow, CalibrationFlowEvent, MIN_SAMPLES_PER_POINT, RawPoint, RawTouchEvent,
    };
    use crate::calibration::CalibrationCorner;

    #[test]
    fn clean_taps_complete_with_averaged_points() {
        let mut calibration_flow = CalibrationFlow::new();

        let calibration_flow_event = run_tap(
            &mut calibration_flow,
            [(100, 200), (101, 201), (102, 202), (103, 203), (104, 204)],
        );
        assert_point_captured(
            calibration_flow_event,
            CalibrationCorner::UpperLeft,
            RawPoint { x: 102, y: 202 },
            CalibrationCorner::UpperRight,
        );

        let calibration_flow_event = run_tap(
            &mut calibration_flow,
            [(900, 210), (901, 211), (902, 212), (903, 213), (904, 214)],
        );
        assert_point_captured(
            calibration_flow_event,
            CalibrationCorner::UpperRight,
            RawPoint { x: 902, y: 212 },
            CalibrationCorner::LowerRight,
        );

        let calibration_flow_event = run_tap(
            &mut calibration_flow,
            [(910, 800), (911, 801), (912, 802), (913, 803), (914, 804)],
        );
        assert_point_captured(
            calibration_flow_event,
            CalibrationCorner::LowerRight,
            RawPoint { x: 912, y: 802 },
            CalibrationCorner::LowerLeft,
        );

        let calibration_flow_event = run_tap(
            &mut calibration_flow,
            [(120, 790), (121, 791), (122, 792), (123, 793), (124, 794)],
        );
        let CalibrationFlowEvent::Completed {
            raw_points,
            calibration_corner,
            usable_sample_count,
            ..
        } = calibration_flow_event
        else {
            panic!("expected completed event");
        };

        assert_eq!(calibration_corner, CalibrationCorner::LowerLeft);
        assert_eq!(usable_sample_count, 4);
        assert_eq!(
            raw_points,
            [
                RawPoint { x: 102, y: 202 },
                RawPoint { x: 902, y: 212 },
                RawPoint { x: 912, y: 802 },
                RawPoint { x: 122, y: 792 },
            ]
        );
    }

    #[test]
    fn held_stylus_dropout_does_not_capture_next_corner_from_old_spot() {
        let mut calibration_flow = CalibrationFlow::new();
        let calibration_flow_event = run_tap(
            &mut calibration_flow,
            [(100, 200), (101, 201), (102, 202), (103, 203), (104, 204)],
        );
        assert_point_captured(
            calibration_flow_event,
            CalibrationCorner::UpperLeft,
            RawPoint { x: 102, y: 202 },
            CalibrationCorner::UpperRight,
        );

        let calibration_flow_event = consume_tap_without_assert(
            &mut calibration_flow,
            [(900, 210), (901, 211), (902, 212), (903, 213), (904, 214)],
        );
        assert_point_captured(
            calibration_flow_event,
            CalibrationCorner::UpperRight,
            RawPoint { x: 902, y: 212 },
            CalibrationCorner::LowerRight,
        );

        assert!(
            calibration_flow
                .handle_raw_touch_event(Some(RawTouchEvent::Down {
                    raw_x: 905,
                    raw_y: 215,
                }))
                .is_none()
        );
        assert!(
            calibration_flow
                .handle_raw_touch_event(Some(RawTouchEvent::Move {
                    raw_x: 906,
                    raw_y: 216,
                }))
                .is_none()
        );
        assert!(
            calibration_flow
                .handle_raw_touch_event(Some(RawTouchEvent::Up))
                .is_none()
        );
        assert_eq!(calibration_flow.calibration_index(), 2);
        assert_eq!(
            calibration_flow.next_corner(),
            Some(CalibrationCorner::LowerRight)
        );

        assert!(calibration_flow.handle_raw_touch_event(None).is_none());

        let calibration_flow_event = run_tap(
            &mut calibration_flow,
            [(910, 800), (911, 801), (912, 802), (913, 803), (914, 804)],
        );
        assert_point_captured(
            calibration_flow_event,
            CalibrationCorner::LowerRight,
            RawPoint { x: 912, y: 802 },
            CalibrationCorner::LowerLeft,
        );
    }

    #[test]
    fn single_tap_without_moves_captures_corner() {
        let mut calibration_flow = CalibrationFlow::new();

        assert!(
            calibration_flow
                .handle_raw_touch_event(Some(RawTouchEvent::Down {
                    raw_x: 100,
                    raw_y: 200,
                }))
                .is_none()
        );
        let calibration_flow_event = calibration_flow
            .handle_raw_touch_event(Some(RawTouchEvent::Up))
            .expect("single tap should capture a calibration point");
        assert_point_captured(
            calibration_flow_event,
            CalibrationCorner::UpperLeft,
            RawPoint { x: 100, y: 200 },
            CalibrationCorner::UpperRight,
        );
        assert!(calibration_flow.handle_raw_touch_event(None).is_none());
    }

    #[test]
    fn move_only_noise_while_armed_is_ignored() {
        let mut calibration_flow = CalibrationFlow::new();

        assert!(
            calibration_flow
                .handle_raw_touch_event(Some(RawTouchEvent::Move {
                    raw_x: 400,
                    raw_y: 500,
                }))
                .is_none()
        );
        assert!(
            calibration_flow
                .handle_raw_touch_event(Some(RawTouchEvent::Up))
                .is_none()
        );
        assert_eq!(calibration_flow.calibration_index(), 0);
        assert_eq!(
            calibration_flow.next_corner(),
            Some(CalibrationCorner::UpperLeft)
        );
    }

    fn run_tap(
        calibration_flow: &mut CalibrationFlow,
        raw_samples: [(u16, u16); 5],
    ) -> CalibrationFlowEvent {
        let calibration_flow_event = consume_tap_without_assert(calibration_flow, raw_samples);
        assert!(calibration_flow.handle_raw_touch_event(None).is_none());
        calibration_flow_event
    }

    fn consume_tap_without_assert(
        calibration_flow: &mut CalibrationFlow,
        raw_samples: [(u16, u16); 5],
    ) -> CalibrationFlowEvent {
        let [
            (down_x, down_y),
            (move1_x, move1_y),
            (move2_x, move2_y),
            (move3_x, move3_y),
            (move4_x, move4_y),
        ] = raw_samples;

        assert!(
            calibration_flow
                .handle_raw_touch_event(Some(RawTouchEvent::Down {
                    raw_x: down_x,
                    raw_y: down_y,
                }))
                .is_none()
        );
        assert!(
            calibration_flow
                .handle_raw_touch_event(Some(RawTouchEvent::Move {
                    raw_x: move1_x,
                    raw_y: move1_y,
                }))
                .is_none()
        );
        assert!(
            calibration_flow
                .handle_raw_touch_event(Some(RawTouchEvent::Move {
                    raw_x: move2_x,
                    raw_y: move2_y,
                }))
                .is_none()
        );
        assert!(
            calibration_flow
                .handle_raw_touch_event(Some(RawTouchEvent::Move {
                    raw_x: move3_x,
                    raw_y: move3_y,
                }))
                .is_none()
        );
        assert!(
            calibration_flow
                .handle_raw_touch_event(Some(RawTouchEvent::Move {
                    raw_x: move4_x,
                    raw_y: move4_y,
                }))
                .is_none()
        );
        calibration_flow
            .handle_raw_touch_event(Some(RawTouchEvent::Up))
            .expect("tap should capture a calibration point")
    }

    fn assert_point_captured(
        calibration_flow_event: CalibrationFlowEvent,
        calibration_corner: CalibrationCorner,
        raw_point: RawPoint,
        next_corner: CalibrationCorner,
    ) {
        let CalibrationFlowEvent::PointCaptured {
            calibration_corner: actual_corner,
            raw_point: actual_raw_point,
            next_corner: actual_next_corner,
            usable_sample_count,
        } = calibration_flow_event
        else {
            panic!("expected point captured event");
        };

        assert_eq!(actual_corner, calibration_corner);
        assert_eq!(actual_raw_point, raw_point);
        assert_eq!(actual_next_corner, next_corner);
        assert!(usable_sample_count >= MIN_SAMPLES_PER_POINT);
    }
}
