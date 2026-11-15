// SwarmForge v1.0.0 — Optional Custom wgpu WGSL Isometric Instanced Shader
// Compatible with wgpu 24.0 for high-density 50,000+ voxel/instance megafactory scenes.

struct CameraUniform {
    view_proj: mat4x4<f32>,
    sun_dir: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct InstanceInput {
    @location(2) offset: vec3<f32>,
    @location(3) scale: vec3<f32>,
    @location(4) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
};

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = v.pos * inst.scale + inst.offset;
    out.clip_pos = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.color = inst.color;
    out.normal = normalize(v.normal);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let ndotl = max(dot(in.normal, normalize(camera.sun_dir.xyz)), 0.0);
    let hemi = 0.42 + 0.18 * in.normal.y;
    let lit = in.color.rgb * (hemi + 0.62 * ndotl);
    return vec4<f32>(lit, in.color.a);
}
