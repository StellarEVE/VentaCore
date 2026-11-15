struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct BlitUniforms {
    transform_matrix: mat4x4<f32>,
    src_rect: vec4<f32>,
    dst_rect: vec4<f32>,
    opacity: f32,
    fit_mode: u32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: BlitUniforms;

@group(0) @binding(1)
var t_diffuse: texture_2d<f32>;

@group(0) @binding(2)
var s_diffuse: sampler;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Full screen triangle covering [-1, 1] x [-1, 1] in NDC
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    let p = positions[in_vertex_index];
    out.clip_position = uniforms.transform_matrix * vec4<f32>(p, 0.0, 1.0);
    // Standard texture coordinate: (0, 0) is top-left, (1, 1) is bottom-right
    out.uv = vec2<f32>((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dst_x0 = uniforms.dst_rect.x;
    let dst_y0 = uniforms.dst_rect.y;
    let dst_w  = uniforms.dst_rect.z;
    let dst_h  = uniforms.dst_rect.w;

    // Check if within active destination bounds
    if (in.uv.x < dst_x0 || in.uv.x > dst_x0 + dst_w ||
        in.uv.y < dst_y0 || in.uv.y > dst_y0 + dst_h) {
        // Outside letterbox frame: solid black border
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    // Normalized coordinate [0, 1] within destination rectangle
    let norm_x = (in.uv.x - dst_x0) / dst_w;
    let norm_y = (in.uv.y - dst_y0) / dst_h;

    // Map into source texture rectangle
    let tex_u = uniforms.src_rect.x + norm_x * uniforms.src_rect.z;
    let tex_v = uniforms.src_rect.y + norm_y * uniforms.src_rect.w;

    let sample_color = textureSample(t_diffuse, s_diffuse, vec2<f32>(tex_u, tex_v));
    return vec4<f32>(sample_color.rgb * uniforms.opacity, sample_color.a * uniforms.opacity);
}
