struct Camera {
    view_projection: mat4x4<f32>,
}

struct ObjectTransform {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<uniform> object: ObjectTransform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    let world_position =
        object.model * vec4<f32>(input.position, 1.0);

    // output.position = vec4<f32>(input.position, 1.0);
    output.position =
        camera.view_projection * world_position;

    // normal use 0.0
    output.normal = (object.normal_matrix * vec4<f32>(input.normal, 0.0)).xyz;
    output.color = input.color;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    var light_factor = 1.0;
    let normal_length_squared = dot(input.normal, input.normal);

    if normal_length_squared > 0.0 {
        let normal = normalize(input.normal);
        let direction_to_light =
            normalize(vec3<f32>(0.5, 1.0, 0.8));

        let diffuse_factor =
            max(dot(normal, direction_to_light), 0.0);

        let ambient_intensity: f32 = 0.2;
        let diffuse_intensity: f32 = 0.8;

        light_factor =
            ambient_intensity
            + diffuse_intensity * diffuse_factor;
    }

    return vec4<f32>(input.color * light_factor, 1.0);
}