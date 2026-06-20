linkage![
    .define_param("x", 0.5)
    .define_param("y", 0.5)
    .define_param("z", 0.5)
    // Left Z rail: stroke from (0,0,0) up to gantry height
    .pen_color(Rgb888::new(85, 85, 85))
    .pen_width(3.0)
    .pen_down()
    .up_param("z", 0.0, 250.0)
    // X gantry rod: stroke from left to right at gantry height
    .mark("gantry")
    .forward(220.0)
    // Right Z rail: stroke from (220,0,z) back down to (220,0,0)
    // up_param with reversed range: movement = 0 + t*(-250) = -z_mm (going down)
    .up_param("z", 0.0, -250.0)
    // Return to gantry-left, then move to X carriage (pen up)
    .restore("gantry")
    .pen_up()
    .forward_param("x", 0.0, 220.0)
    // X carriage sphere
    .pen_color(Rgb888::new(51, 51, 51))
    .sphere(8.0)
    // Y arm: stroke from X carriage to nozzle (bed Y motion)
    .pen_color(Rgb888::new(85, 85, 85))
    .pen_width(2.0)
    .pen_down()
    .left_param("y", 0.0, 220.0)
    // Nozzle sphere
    .pen_up()
    .pen_color(Rgb888::new(230, 57, 70))
    .sphere(6.0)
]
