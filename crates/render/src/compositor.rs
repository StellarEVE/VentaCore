//! GPU Compositor powered by wgpu and WGSL shaders.
//!
//! Handles offscreen video rendering, transitions, 2D transforms, and fit modes.
//! Provides automatic graceful fallback to CpuReferenceOracle when hardware acceleration
//! is absent or unsupported (Charter Rule 5).

use crate::error::RenderError;
use crate::oracle::CpuReferenceOracle;
use crate::shaders::{BLIT_SHADER, TRANSITION_SHADER};
use crate::types::{FitMode, RenderFrame, Transform2D, TransitionKind, TransitionParams};
use bytemuck::{Pod, Zeroable};
use std::sync::mpsc;
use wgpu::util::DeviceExt;

/// Uniform buffer for blit/transform/fit rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct BlitUniforms {
    pub transform_matrix: [f32; 16],
    pub src_rect: [f32; 4],
    pub dst_rect: [f32; 4],
    pub opacity: f32,
    pub fit_mode: u32,
    pub _pad0: f32,
    pub _pad1: f32,
}

/// Uniform buffer for transition rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TransitionUniforms {
    pub progress: f32,
    pub kind: u32,
    pub _pad0: f32,
    pub _pad1: f32,
    pub dip_color: [f32; 4],
}

/// GPU Compositor instance managing wgpu pipelines and offscreen render targets.
pub struct GpuCompositor {
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    adapter_info: Option<wgpu::AdapterInfo>,
    blit_pipeline: Option<wgpu::RenderPipeline>,
    blit_bind_group_layout: Option<wgpu::BindGroupLayout>,
    transition_pipeline: Option<wgpu::RenderPipeline>,
    transition_bind_group_layout: Option<wgpu::BindGroupLayout>,
    sampler: Option<wgpu::Sampler>,
    oracle: CpuReferenceOracle,
    gpu_available: bool,
}

impl GpuCompositor {
    /// Creates a new compositor. Attempts to initialize wgpu graphics pipeline;
    /// if unavailable (e.g. headless CI without GPU), gracefully falls back to CPU oracle.
    pub fn new() -> Self {
        let oracle = CpuReferenceOracle::new();
        match Self::try_init_gpu() {
            Ok((device, queue, adapter_info, blit_pipeline, blit_bgl, trans_pipeline, trans_bgl, sampler)) => {
                Self {
                    device: Some(device),
                    queue: Some(queue),
                    adapter_info: Some(adapter_info),
                    blit_pipeline: Some(blit_pipeline),
                    blit_bind_group_layout: Some(blit_bgl),
                    transition_pipeline: Some(trans_pipeline),
                    transition_bind_group_layout: Some(trans_bgl),
                    sampler: Some(sampler),
                    oracle,
                    gpu_available: true,
                }
            }
            Err(_) => Self {
                device: None,
                queue: None,
                adapter_info: None,
                blit_pipeline: None,
                blit_bind_group_layout: None,
                transition_pipeline: None,
                transition_bind_group_layout: None,
                sampler: None,
                oracle,
                gpu_available: false,
            },
        }
    }

    /// Creates a compositor strictly configured to use the CPU software reference oracle.
    pub fn new_cpu_only() -> Self {
        Self {
            device: None,
            queue: None,
            adapter_info: None,
            blit_pipeline: None,
            blit_bind_group_layout: None,
            transition_pipeline: None,
            transition_bind_group_layout: None,
            sampler: None,
            oracle: CpuReferenceOracle::new(),
            gpu_available: false,
        }
    }

    /// Returns true if hardware acceleration is active.
    pub fn is_hardware_accelerated(&self) -> bool {
        self.gpu_available
    }

    /// Returns the active adapter name if GPU is initialized.
    pub fn adapter_name(&self) -> Option<String> {
        self.adapter_info.as_ref().map(|info| info.name.clone())
    }

    /// Returns a reference to the CPU Reference Oracle.
    pub fn oracle(&self) -> &CpuReferenceOracle {
        &self.oracle
    }

    /// Renders a single frame with fit mode and 2D transform onto a target canvas.
    /// Uses GPU acceleration if available, falling back to CPU reference oracle otherwise.
    pub fn render_cut(
        &mut self,
        source: &RenderFrame,
        target_width: u32,
        target_height: u32,
        fit: FitMode,
        transform: &Transform2D,
    ) -> Result<RenderFrame, RenderError> {
        if self.gpu_available {
            match self.render_cut_gpu(source, target_width, target_height, fit, transform) {
                Ok(frame) => return Ok(frame),
                Err(err) => {
                    // Fall back to CPU oracle on GPU execution failure (Charter Rule 5)
                    eprintln!("GPU cut render failed: {err}; using CPU reference fallback.");
                }
            }
        }
        Ok(self.oracle.render_cut(source, target_width, target_height, fit, transform))
    }

    /// Renders a transition between two frames.
    /// Uses GPU acceleration if available, falling back to CPU reference oracle otherwise.
    pub fn render_transition(
        &mut self,
        from_frame: &RenderFrame,
        to_frame: &RenderFrame,
        target_width: u32,
        target_height: u32,
        fit: FitMode,
        params: &TransitionParams,
    ) -> Result<RenderFrame, RenderError> {
        if self.gpu_available {
            match self.render_transition_gpu(from_frame, to_frame, target_width, target_height, fit, params) {
                Ok(frame) => return Ok(frame),
                Err(err) => {
                    eprintln!("GPU transition render failed: {err}; using CPU reference fallback.");
                }
            }
        }
        Ok(self.oracle.render_transition(from_frame, to_frame, target_width, target_height, fit, params))
    }

    /// Executes render_cut strictly on GPU hardware. Fails if GPU is not available.
    pub fn render_cut_gpu(
        &mut self,
        source: &RenderFrame,
        target_width: u32,
        target_height: u32,
        fit: FitMode,
        transform: &Transform2D,
    ) -> Result<RenderFrame, RenderError> {
        let device = self.device.as_ref().ok_or(RenderError::AdapterNotFound)?;
        let queue = self.queue.as_ref().ok_or(RenderError::AdapterNotFound)?;
        let pipeline = self.blit_pipeline.as_ref().ok_or(RenderError::AdapterNotFound)?;
        let bind_group_layout = self.blit_bind_group_layout.as_ref().ok_or(RenderError::AdapterNotFound)?;
        let sampler = self.sampler.as_ref().ok_or(RenderError::AdapterNotFound)?;

        if target_width == 0 || target_height == 0 || source.width == 0 || source.height == 0 {
            return Err(RenderError::InvalidDimensions {
                width: target_width,
                height: target_height,
            });
        }

        // For BlurPadding on GPU: use CPU oracle for now or composite background
        if fit == FitMode::BlurPadding {
            return Ok(self.oracle.render_cut(source, target_width, target_height, fit, transform));
        }

        // Compute UV mapping rectangles
        let (src_rect, dst_rect, fit_mode_u32) = calculate_rects(source.width, source.height, target_width, target_height, fit);

        let uniforms = BlitUniforms {
            transform_matrix: build_transform_matrix(transform),
            src_rect,
            dst_rect,
            opacity: transform.opacity.clamp(0.0, 1.0),
            fit_mode: fit_mode_u32,
            _pad0: 0.0,
            _pad1: 0.0,
        };

        // 1. Upload uniform buffer
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Blit Uniforms Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // 2. Upload source texture
        let source_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Blit Source Texture"),
            size: wgpu::Extent3d {
                width: source.width,
                height: source.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &source_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &source.pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(source.width * 4),
                rows_per_image: Some(source.height),
            },
            wgpu::Extent3d {
                width: source.width,
                height: source.height,
                depth_or_array_layers: 1,
            },
        );

        let source_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 3. Create BindGroup
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blit Bind Group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        // 4. Create Offscreen Render Target Texture
        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Blit Render Target"),
            size: wgpu::Extent3d {
                width: target_width,
                height: target_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let render_view = render_target.create_view(&wgpu::TextureViewDescriptor::default());

        // 5. Setup Staging Readback Buffer
        let unpadded_bytes_per_row = target_width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) & !(align - 1);
        let buffer_size = (padded_bytes_per_row * target_height) as wgpu::BufferAddress;

        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Blit Readback Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // 6. Encode Render Pass and Readback Copy
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Blit Command Encoder"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Blit Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.draw(0..3, 0..1);
        }

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &readback_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(target_height),
                },
            },
            wgpu::Extent3d {
                width: target_width,
                height: target_height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(Some(encoder.finish()));

        // 7. Map buffer and read back pixels
        let slice = readback_buffer.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|e| RenderError::BufferMappingFailed(e.to_string()))?
            .map_err(|e| RenderError::BufferMappingFailed(format!("{:?}", e)))?;

        let mapped = slice.get_mapped_range();
        let mut output_pixels = vec![0u8; (target_width * target_height * 4) as usize];
        for y in 0..target_height {
            let src_start = (y * padded_bytes_per_row) as usize;
            let src_end = src_start + (target_width * 4) as usize;
            let dst_start = (y * target_width * 4) as usize;
            let dst_end = dst_start + (target_width * 4) as usize;
            output_pixels[dst_start..dst_end].copy_from_slice(&mapped[src_start..src_end]);
        }
        drop(mapped);
        readback_buffer.unmap();

        RenderFrame::from_rgba8(target_width, target_height, output_pixels)
    }

    /// Executes render_transition strictly on GPU hardware. Fails if GPU is not available.
    pub fn render_transition_gpu(
        &mut self,
        from_frame: &RenderFrame,
        to_frame: &RenderFrame,
        target_width: u32,
        target_height: u32,
        fit: FitMode,
        params: &TransitionParams,
    ) -> Result<RenderFrame, RenderError> {
        if target_width == 0 || target_height == 0 {
            return Err(RenderError::InvalidDimensions {
                width: target_width,
                height: target_height,
            });
        }

        // Fast shortcut for boundary conditions
        if params.progress <= 0.0 {
            return self.render_cut_gpu(from_frame, target_width, target_height, fit, &Transform2D::default());
        }
        if params.progress >= 1.0 {
            return self.render_cut_gpu(to_frame, target_width, target_height, fit, &Transform2D::default());
        }

        // First render from_frame and to_frame onto target dimensions with fit applied
        let canvas_a = self.render_cut(from_frame, target_width, target_height, fit, &Transform2D::default())?;
        let canvas_b = self.render_cut(to_frame, target_width, target_height, fit, &Transform2D::default())?;

        let device = self.device.as_ref().ok_or(RenderError::AdapterNotFound)?;
        let queue = self.queue.as_ref().ok_or(RenderError::AdapterNotFound)?;
        let pipeline = self.transition_pipeline.as_ref().ok_or(RenderError::AdapterNotFound)?;
        let bind_group_layout = self.transition_bind_group_layout.as_ref().ok_or(RenderError::AdapterNotFound)?;
        let sampler = self.sampler.as_ref().ok_or(RenderError::AdapterNotFound)?;

        let (kind_u32, dip_color) = match params.kind {
            TransitionKind::None => (0u32, [0.0, 0.0, 0.0, 1.0]),
            TransitionKind::CrossDissolve => (1u32, [0.0, 0.0, 0.0, 1.0]),
            TransitionKind::DipToColor(c) => (
                2u32,
                [
                    c[0] as f32 / 255.0,
                    c[1] as f32 / 255.0,
                    c[2] as f32 / 255.0,
                    c[3] as f32 / 255.0,
                ],
            ),
            TransitionKind::WipeLeft => (3u32, [0.0, 0.0, 0.0, 1.0]),
            TransitionKind::WipeRight => (4u32, [0.0, 0.0, 0.0, 1.0]),
            TransitionKind::PushLeft => (5u32, [0.0, 0.0, 0.0, 1.0]),
            TransitionKind::PushRight => (6u32, [0.0, 0.0, 0.0, 1.0]),
        };

        let uniforms = TransitionUniforms {
            progress: params.progress.clamp(0.0, 1.0),
            kind: kind_u32,
            _pad0: 0.0,
            _pad1: 0.0,
            dip_color,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Transition Uniforms Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Upload texture A
        let texture_a = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Transition Texture A"),
            size: wgpu::Extent3d {
                width: target_width,
                height: target_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture_a,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &canvas_a.pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(target_width * 4),
                rows_per_image: Some(target_height),
            },
            wgpu::Extent3d {
                width: target_width,
                height: target_height,
                depth_or_array_layers: 1,
            },
        );
        let view_a = texture_a.create_view(&wgpu::TextureViewDescriptor::default());

        // Upload texture B
        let texture_b = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Transition Texture B"),
            size: wgpu::Extent3d {
                width: target_width,
                height: target_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture_b,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &canvas_b.pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(target_width * 4),
                rows_per_image: Some(target_height),
            },
            wgpu::Extent3d {
                width: target_width,
                height: target_height,
                depth_or_array_layers: 1,
            },
        );
        let view_b = texture_b.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Transition Bind Group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        // Output render target
        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Transition Render Target"),
            size: wgpu::Extent3d {
                width: target_width,
                height: target_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let render_view = render_target.create_view(&wgpu::TextureViewDescriptor::default());

        let unpadded_bytes_per_row = target_width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) & !(align - 1);
        let buffer_size = (padded_bytes_per_row * target_height) as wgpu::BufferAddress;

        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Transition Readback Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Transition Command Encoder"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Transition Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.draw(0..3, 0..1);
        }

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &readback_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(target_height),
                },
            },
            wgpu::Extent3d {
                width: target_width,
                height: target_height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(Some(encoder.finish()));

        let slice = readback_buffer.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|e| RenderError::BufferMappingFailed(e.to_string()))?
            .map_err(|e| RenderError::BufferMappingFailed(format!("{:?}", e)))?;

        let mapped = slice.get_mapped_range();
        let mut output_pixels = vec![0u8; (target_width * target_height * 4) as usize];
        for y in 0..target_height {
            let src_start = (y * padded_bytes_per_row) as usize;
            let src_end = src_start + (target_width * 4) as usize;
            let dst_start = (y * target_width * 4) as usize;
            let dst_end = dst_start + (target_width * 4) as usize;
            output_pixels[dst_start..dst_end].copy_from_slice(&mapped[src_start..src_end]);
        }
        drop(mapped);
        readback_buffer.unmap();

        RenderFrame::from_rgba8(target_width, target_height, output_pixels)
    }

    #[allow(clippy::type_complexity)]
    fn try_init_gpu() -> Result<
        (
            wgpu::Device,
            wgpu::Queue,
            wgpu::AdapterInfo,
            wgpu::RenderPipeline,
            wgpu::BindGroupLayout,
            wgpu::RenderPipeline,
            wgpu::BindGroupLayout,
            wgpu::Sampler,
        ),
        RenderError,
    > {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::empty(),
            dx12_shader_compiler: wgpu::Dx12Compiler::default(),
            gles_minor_version: wgpu::Gles3MinorVersion::default(),
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .or_else(|| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: true,
            }))
        })
        .ok_or(RenderError::AdapterNotFound)?;

        let adapter_info = adapter.get_info();

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("VentaCore Render Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
            },
            None,
        ))
        .map_err(|e| RenderError::DeviceRequestFailed(e.to_string()))?;

        // Compile blit shader
        let blit_shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blit WGSL Shader"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
        });

        // Blit bind group layout
        let blit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Blit Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blit Pipeline Layout"),
            bind_group_layouts: &[&blit_bgl],
            push_constant_ranges: &[],
        });

        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blit Render Pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader_module,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Compile transition shader
        let trans_shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Transition WGSL Shader"),
            source: wgpu::ShaderSource::Wgsl(TRANSITION_SHADER.into()),
        });

        let trans_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Transition Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let trans_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Transition Pipeline Layout"),
            bind_group_layouts: &[&trans_bgl],
            push_constant_ranges: &[],
        });

        let trans_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Transition Render Pipeline"),
            layout: Some(&trans_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &trans_shader_module,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &trans_shader_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Linear Bilinear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok((
            device,
            queue,
            adapter_info,
            blit_pipeline,
            blit_bgl,
            trans_pipeline,
            trans_bgl,
            sampler,
        ))
    }
}

impl Default for GpuCompositor {
    fn default() -> Self {
        Self::new()
    }
}

fn calculate_rects(
    src_w: u32,
    src_h: u32,
    tgt_w: u32,
    tgt_h: u32,
    fit: FitMode,
) -> ([f32; 4], [f32; 4], u32) {
    let ar_src = src_w as f32 / src_h as f32;
    let ar_tgt = tgt_w as f32 / tgt_h as f32;

    match fit {
        FitMode::Crop => {
            let (crop_w, crop_h, crop_u0, crop_v0) = if ar_src >= ar_tgt {
                let ch = src_h as f32;
                let cw = ch * ar_tgt;
                let u0 = (src_w as f32 - cw) * 0.5;
                (cw, ch, u0, 0.0)
            } else {
                let cw = src_w as f32;
                let ch = cw / ar_tgt;
                let v0 = (src_h as f32 - ch) * 0.5;
                (cw, ch, 0.0, v0)
            };
            (
                [
                    crop_u0 / src_w as f32,
                    crop_v0 / src_h as f32,
                    crop_w / src_w as f32,
                    crop_h / src_h as f32,
                ],
                [0.0, 0.0, 1.0, 1.0],
                0,
            )
        }
        FitMode::Letterbox | FitMode::BlurPadding => {
            let (fit_w, fit_h, offset_x, offset_y) = if ar_src >= ar_tgt {
                let fw = tgt_w as f32;
                let fh = (fw / ar_src).round();
                let oy = ((tgt_h as f32 - fh) * 0.5).floor();
                (fw, fh, 0.0, oy)
            } else {
                let fh = tgt_h as f32;
                let fw = (fh * ar_src).round();
                let ox = ((tgt_w as f32 - fw) * 0.5).floor();
                (fw, fh, ox, 0.0)
            };
            (
                [0.0, 0.0, 1.0, 1.0],
                [
                    offset_x / tgt_w as f32,
                    offset_y / tgt_h as f32,
                    fit_w / tgt_w as f32,
                    fit_h / tgt_h as f32,
                ],
                1,
            )
        }
    }
}

fn build_transform_matrix(transform: &Transform2D) -> [f32; 16] {
    let cos_r = transform.rotation_radians.cos();
    let sin_r = transform.rotation_radians.sin();
    let sx = transform.scale[0];
    let sy = transform.scale[1];
    let tx = transform.position[0];
    let ty = transform.position[1];

    // Column-major 4x4 matrix
    [
        sx * cos_r,   sx * sin_r,  0.0, 0.0, // Col 0
        -sy * sin_r,  sy * cos_r,  0.0, 0.0, // Col 1
        0.0,          0.0,         1.0, 0.0, // Col 2
        tx,           ty,          0.0, 1.0, // Col 3
    ]
}
