use super::{
    CALIBRATION_POINT_COUNT, CalibrationConfig, CalibrationCorner, RawPoint, RawTouchEvent,
    calibration_corner_for_index,
};

/// Sans-io state machine for the four-tap calibration flow.
///
/// Callers own I/O: they draw [`CalibrationCorner`] crosses, log progress, and
/// persist the finished [`CalibrationConfig`]. This flow only tracks which
/// corner is next, records raw touch-down samples, and computes the affine map
/// once all four corners are collected.
pub struct CalibrationFlow {
    calibration_index: usize,
    calibration_points: [RawPoint; CALIBRATION_POINT_COUNT],
}

impl CalibrationFlow {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            calibration_index: 0,
            calibration_points: [RawPoint { x: 0, y: 0 }; CALIBRATION_POINT_COUNT],
        }
    }

    pub fn restart(&mut self) {
        self.calibration_index = 0;
        self.calibration_points = [RawPoint { x: 0, y: 0 }; CALIBRATION_POINT_COUNT];
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
        let Some(raw_touch_event) = raw_touch_event else {
            return None;
        };
        let Some(calibration_corner) = self.next_corner() else {
            return None;
        };

        let RawTouchEvent::Down { raw_x, raw_y } = raw_touch_event else {
            return None;
        };

        let raw_point = RawPoint { x: raw_x, y: raw_y };
        self.calibration_points[self.calibration_index] = raw_point;
        self.calibration_index += 1;

        if self.calibration_index == CALIBRATION_POINT_COUNT {
            let raw_points = self.calibration_points;
            let calibration_config = CalibrationConfig::from_four_points(raw_points);
            return Some(CalibrationFlowEvent::Completed {
                calibration_config,
                raw_points,
            });
        }

        Some(CalibrationFlowEvent::PointCaptured {
            calibration_corner,
            raw_point,
            next_corner: self
                .next_corner()
                .expect("next corner exists until calibration completes"),
        })
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
    },
    Completed {
        calibration_config: CalibrationConfig,
        raw_points: [RawPoint; CALIBRATION_POINT_COUNT],
    },
}
