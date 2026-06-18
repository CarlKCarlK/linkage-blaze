# Linkage-Like Systems Across Domains

Many domains use the same underlying idea: a sequence or graph of transforms, joints, poses, or constraints that produces geometry or motion. Depending on the field, these are called linkages, kinematic chains, skeletons, rigs, scene graphs, toolpaths, or transform hierarchies.

Useful umbrella terms include:

- **Kinematic chain**
- **Articulated model**
- **Transform hierarchy**
- **Pose graph**
- **Linkage**
- **Rig**

## Domains and common terms

| Domain | Common names | Notes |
|---|---|---|
| Robotics | Linkage, manipulator, robot arm, kinematic chain, forward kinematics, inverse kinematics | Classic serial arms, grippers, robot legs, calibration, and tool positioning. |
| Mechanical engineering | Linkage, mechanism, four-bar linkage, crank-rocker, cam and follower | Physical parts connected by joints and constraints. Often analyzed for motion, force, and manufacturability. |
| Turtle graphics | Turtle, pen plotter, Logo turtle, turtle geometry | Sequential movement and rotation commands create paths. Very close to a command-chain linkage model. |
| CNC and fabrication | Toolpath, G-code, machine kinematics, postprocessor | Move and orient a tool head through space. Multi-axis machines add orientation control. |
| 3D modeling: object placement | Transform hierarchy, scene graph, parent-child transforms | Objects inherit transforms from parent nodes. Common in CAD, Blender, Three.js, and game engines. |
| 3D modeling: figure control | Skeleton, armature, bones, rigging, skinning, pose | Character joints form a hierarchy. Mesh vertices are deformed by bones. |
| Animation | Rig, animation rig, pose, keyframes, constraints | Time-varying versions of skeletons and transform hierarchies, often with constraints and inverse kinematics. |
| Games | Scene graph, transform hierarchy, skeletal animation, inverse kinematics, ragdoll | Used for characters, weapons, vehicles, cameras, and procedural animation. |
| 3D camera control | Camera rig, orbit camera, dolly, truck, pedestal, pan, tilt, roll | A virtual camera is a body with a pose. Controls map naturally to yaw, pitch, roll, and translation. |
| Physical camera and cinematography | Motion-control rig, pan-tilt head, gimbal, dolly, crane, jib, slider | Real hardware version of camera kinematics. Often programmed for repeatable camera paths. |
| Drones and vehicles | Pose, attitude, yaw/pitch/roll, trajectory, gimbal control | Vehicle pose plus attached camera or sensor gimbals. |
| CAD assemblies | Assembly constraints, mates, joints, articulated assembly | Parts connected by constraints. Useful for simulating mechanical assemblies. |
| Physics simulation | Rigid bodies, joints, constraints, articulated body dynamics | Linkage-like structures plus mass, inertia, forces, and collision. |
| Biomechanics | Skeletal model, musculoskeletal model, gait model, joint angles | Human and animal bodies modeled as articulated chains with anatomical constraints. |
| Medical and surgical planning | Articulated anatomy, instrument kinematics, surgical robot kinematics | Includes bones, joints, scopes, catheters, and robot-assisted surgical tools. |
| Computer vision and pose estimation | Pose graph, skeleton tracking, keypoints, articulated pose | Estimate body, hand, or object joint positions from images or video. |
| AR and VR | Tracking rig, controller pose, hand skeleton, avatar rig | Head, hands, controllers, and avatar bones form linked pose systems. |
| Aerospace | Attitude, reference frames, articulated appendages, gimbals | Spacecraft attitude plus solar panels, robotic arms, antennas, and sensor gimbals. |
| Cranes and industrial equipment | Crane kinematics, boom, jib, cable, manipulator | Large-scale physical linkages with load limits and constraints. |
| Architecture and kinetic sculpture | Kinetic structure, deployable mechanism, transformable structure | Moving walls, roofs, shades, sculptures, and folding structures. |
| Origami and folding structures | Crease pattern, folding kinematics, deployable linkage | Flat panels connected by hinge-like folds. Useful for deployable structures. |
| Molecular modeling | Molecular conformation, torsion angles, kinematic chain | Atoms connected by bonds. Rotations around bonds create conformations. |
| Protein modeling | Backbone torsion, side-chain rotamers, conformation | Linkage-like chains at molecular scale. |
| Puppetry and animatronics | Puppet rig, control rig, servo linkage, animatronic skeleton | Physical or virtual characters controlled through joints and linkages. |
| Stage automation and lighting | Lighting rig, pan-tilt fixture, stage motion control | Moving lights, cameras, props, screens, and scenery use controllable articulated poses. |
| UI and data visualization | Node-link diagram, graph layout, constraint layout | Less physical, but often uses linked constraints and transformations. |

## Compact taxonomy

### Geometry-only systems

These care mostly about shape and pose, not mass or forces.

- Turtle graphics
- Pen plotters
- Scene graphs
- Transform hierarchies
- Skeletons and rigs
- 3D camera rigs

### Physical kinematics

These model real motion but often ignore dynamics.

- Robot arms
- CNC machines
- Camera motion-control rigs
- Cranes
- Mechanical linkages
- Gimbals
- Drones and vehicles

### Dynamics and simulation

These add mass, inertia, forces, and collision.

- Articulated rigid bodies
- Ragdolls
- Biomechanical models
- Physics engines
- Vehicle simulators

### Constraint-based systems

These are defined less by a simple command sequence and more by constraints among parts.

- CAD assemblies
- Inverse-kinematics rigs
- Folding structures
- Origami mechanisms
- Deployable structures

### Biological and molecular systems

These use linkage-like geometry at body scale or molecular scale.

- Human and animal skeletons
- Gait models
- Surgical instruments
- Molecular conformations
- Protein backbone and side-chain torsions

## Notes for Linkage Blaze

Linkage Blaze fits naturally into several of these categories:

- As **turtle graphics**, it is a sequence of movement and rotation commands.
- As **robotics**, it resembles forward kinematics for a serial kinematic chain.
- As **3D graphics**, it resembles a transform hierarchy or scene graph.
- As **animation**, parameters act like rig controls.
- As **camera control**, yaw/pitch/roll and forward/left/up match common virtual-camera controls.

The broadest documentation phrase is probably:

> Linkage Blaze describes articulated geometry using turtle-like commands, producing a parameterized kinematic chain that can be rendered, animated, or analyzed.
