struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct TransitionUniforms {
    progress: f32,
    kind: u32,
    _pad0: f32,
    _pad1: f32,
    dip_color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: TransitionUniforms;

@group(0) @binding(1)
var t_from: texture_2d<f32>;

@group(0) @binding(2)
var t_to: texture_2d<f32>;

@group(0) @binding(3)
var s_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    let p = positions[in_vertex_index];
    out.clip_position = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = clamp(uniforms.progress, 0.0, 1.0);

    // 0: None (Hard Cut at 50%)
    if (uniforms.kind == 0u) {
        if (t < 0.5) {
            return textureSample(t_from, s_sampler, in.uv);
        } else {
            return textureSample(t_to, s_sampler, in.uv);
        }
    }

    // 1: Cross Dissolve
    if (uniforms.kind == 1u) {
        let col_from = textureSample(t_from, s_sampler, in.uv);
        let col_to = textureSample(t_to, s_sampler, in.uv);
        return mix(col_from, col_to, t);
    }

    // 2: Dip to Color
    if (uniforms.kind == 2u) {
        if (t <= 0.5) {
            let col_from = textureSample(t_from, s_sampler, in.uv);
            return mix(col_from, uniforms.dip_color, t * 2.0);
        } else {
            let col_to = textureSample(t_to, s_sampler, in.uv);
            return mix(uniforms.dip_color, col_to, (t - 0.5) * 2.0);
        }
    }

    // 3: Wipe Left
    if (uniforms.kind == 3u) {
        if (in.uv.x < t) {
            return textureSample(t_to, s_sampler, in.uv);
        } else {
            return textureSample(t_from, s_sampler, in.uv);
        }
    }

    // 4: Wipe Right
    if (uniforms.kind == 4u) {
        if (in.uv.x >= 1.0 - t) {
            return textureSample(t_to, s_sampler, in.uv);
        } else {
            return textureSample(t_from, s_sampler, in.uv);
        }
    }

    // 5: Push Left
    if (uniforms.kind == 5u) {
        let shift = t;
        if (in.uv.x + shift < 1.0) {
            return textureSample(t_from, s_sampler, vec2<f32>(in.uv.x + shift, in.uv.y));
        } else {
            return textureSample(t_to, s_sampler, vec2<f32>(in.uv.x - (1.0 - shift), in.uv.y));
        }
    }

    // 6: Push Right
    if (uniforms.kind == 6u) {
        let shift = t;
        if (in.uv.x >= shift) {
            return textureSample(t_from, s_sampler, vec2<f32>(in.uv.x - shift, in.uv.y));
        } else {
            return textureSample(t_to, s_sampler, vec2<f32>(in.uv.x + (1.0 - shift), in.uv.y));
        }
    }

    return textureSample(t_from, s_sampler, in.uv);
}
