Linkage::start()
    .define_param("x/y view", 0.58333)
    .define_param("z", 0.39)
    .pen_up()
    .up_param("z", -6.0, 0.0)
    .pitch_param("z", 0.0, -45.0)
    .yaw_param("x/y view", 180.0, -180.0)
    .pen_down()
    