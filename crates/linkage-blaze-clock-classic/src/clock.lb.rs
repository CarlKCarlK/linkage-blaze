linkage![
    .define_param("hour", 0.0)
    .define_param("face spin", 0.5)
    // Common transform for the whole clock face.
    .roll_param("face spin", -90.0, 90.0)
    .mark("face")
    // Face disk
    .pen_color(Rgb888::new(33, 79, 155))
    .disk(66.0)
    // 12 o'clock tick
    .restore("face")
    .pen_width(4.5)
    .pen_color(Rgb888::new(255, 245, 216))
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
    .pen_color(Rgb888::new(250, 235, 215))
    .pen_width(16.0)
    .yaw_param("hour", 360.0, 0.0)
    .forward(40.0)
    // Minute hand
    .restore("face")
    .pen_color(Rgb888::new(69, 215, 255))
    .pen_width(7.5)
    .yaw_param("hour", 4320.0, 0.0)
    .forward(52.0)
    // Second hand
    .restore("face")
    .pen_color(Rgb888::new(255, 89, 72))
    .pen_width(2.0)
    .yaw_param("hour", 259_200.0, 0.0)
    .forward(60.0)
    // Hub
    .restore("face")
    .pen_color(Rgb888::new(255, 0, 0))
    .disk(8.0)
]
