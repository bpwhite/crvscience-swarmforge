// Copyright (C) 2026 CRV Science Network (https://crvscience.com)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://gnu.org>.

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
    @location(2) world_pos: vec3<f32>,
};

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = v.pos * inst.scale + inst.offset;
    out.clip_pos = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.color = inst.color;
    let safe_scale = max(abs(inst.scale), vec3<f32>(1e-4, 1e-4, 1e-4));
    out.normal = normalize(v.normal / safe_scale);
    out.world_pos = world_pos;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let l = normalize(camera.sun_dir.xyz);
    let ndotl = max(dot(n, l), 0.0);
    let hemi = 0.42 + 0.18 * n.y;
    let view_dir = normalize(vec3<f32>(0.57735, 0.57735, 0.57735));
    let half_vec = normalize(l + view_dir);
    let spec = pow(max(dot(n, half_vec), 0.0), 28.0) * 0.24;
    let fresnel = pow(1.0 - max(dot(n, view_dir), 0.0), 3.0) * 0.16;
    let lit = in.color.rgb * (hemi + 0.62 * ndotl) + vec3<f32>(spec + fresnel * 0.75, spec + fresnel * 0.90, spec + fresnel);
    return vec4<f32>(lit, in.color.a);
}
