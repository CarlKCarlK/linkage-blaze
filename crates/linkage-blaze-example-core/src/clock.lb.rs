linkage![
    .define_param("hour", 0.0)
    .define_param("face spin", 0.5)
    // Common transform for the whole clock face.
    .roll_param("face spin", -90.0, 90.0)
    .mark("face")
    // Face rim
    .pen_color(Rgb888::new(178, 124, 44)) // muted brass gold (178, 124, 44)
    .disk(67.0)
    // Face disk
    .pen_color(Rgb888::new(24, 62, 118)) // desaturated deep blue (24, 62, 118)
    .disk(65.0)
    // 12 o'clock tick
    .restore("face")
    .pen_width(3.25)
    .pen_color(Rgb888::new(230, 195, 115)) // muted pale gold (230, 195, 115)
    .pen_up()
    .mark("ticks")
    .forward(45.0)
    .pen_down()
    .forward(18.0)
    // 3 o'clock tick
    .restore("ticks")
    .yaw(-90.0)
    .forward(51.0)
    .pen_down()
    .forward(11.0)
    // 6 o'clock tick
    .restore("ticks")
    .yaw(180.0)
    .forward(51.0)
    .pen_down()
    .forward(11.0)
    // 9 o'clock tick
    .restore("ticks")
    .yaw(90.0)
    .forward(51.0)
    .pen_down()
    .forward(11.0)
    // Hour hand
    .restore("face")
    .pen_color(Rgb888::new(245, 220, 165)) // warm brass ivory (245, 220, 165)
    .pen_width(10.5)
    .yaw_param("hour", 360.0, 0.0)
    .forward(40.0)
    // Minute hand
    .restore("face")
    .pen_color(Rgb888::new(96, 205, 220)) // softened blue-green (96, 205, 220)
    .pen_width(6.0)
    .yaw_param("hour", 4320.0, 0.0)
    .forward(52.0)
    // Second hand
    .restore("face")
    .pen_color(Rgb888::new(230, 95, 70)) // muted coral red (230, 95, 70)
    .pen_width(2.0)
    .yaw_param("hour", 259_200.0, 0.0)
    .forward(60.0)
    // Hub
    .restore("face")
    .pen_color(Rgb888::new(235, 85, 62)) // muted red-orange (235, 85, 62)
    .sphere(7.0)
]
