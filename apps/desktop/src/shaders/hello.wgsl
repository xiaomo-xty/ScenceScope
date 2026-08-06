struct Camera {
    view_projection: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

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

    // output.position = vec4<f32>(input.position, 1.0);
    output.position =
        camera.view_projection * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.color = input.color;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    var light_factor = 1.0;
    let normal_length_squared = dot(input.normal, input.normal);

    if normal_length_squared > 0.0 {
        let normal = normalize(input.normal);
        let light_direction = normalize(vec3<f32>(0.5, 1.0, 0.8));
        let diffuse = max(dot(normal, light_direction), 0.0);

        light_factor = 0.2 + 0.8 * diffuse;
    }

    return vec4<f32>(input.color * light_factor, 1.0);
}