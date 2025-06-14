use glam::{IVec3, USizeVec3, Vec3, Vec4};
use save_format::byte::{ByteReader, ByteWriter};


pub const VOXEL_MESH_MAGIC : [u8; 10] = *b"VOXEL_MESH";
pub const VOXEL_MESH_VERSION : [u8; 4] = [0, 0, 0, 1];


#[derive(PartialEq, Debug, Clone, Copy)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub rgba: u32,
}

#[derive(Debug, Clone, Copy)]
///! plane data with 4 vertices
pub struct Quad {
    pub colour: u32,
    pub corners: [Vec3; 4],
}

#[derive(Debug)]
pub struct Voxel {
    pub pos: IVec3,
    pub colour: u32,
}


#[derive(PartialEq, Debug)]
pub struct VoxelMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}


#[derive(Debug)]
pub enum VoxelMeshDecodeError {
    InvalidByteWriter,
    InvalidMagicValue,
    EOI,
    InvalidVersion {
        lib_version: [u8; 4],
        file_version: [u8; 4],
    }
}


pub fn draw_quad(verticies: &mut Vec<Vertex>, indicies: &mut Vec<u32>, quad: Quad) {
    let k = verticies.len() as u32;
    for corner in quad.corners {
        let colour = quad.colour;
        verticies.push(Vertex::new(Vec3::new(corner[0] as f32, corner[1] as f32, corner[2] as f32), colour));
    }


    indicies.extend_from_slice(&[k, k+1, k+2, k+2, k+3, k]);
}



impl VoxelMesh {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = ByteWriter::new();
        writer.write(VOXEL_MESH_MAGIC); // magik
        writer.write(VOXEL_MESH_VERSION); // version

        writer.write_u32(self.vertices.len() as _);

        for vertex in &self.vertices {
            writer.write_f32(vertex.position.x);
            writer.write_f32(vertex.position.y);
            writer.write_f32(vertex.position.z);
            writer.write_u32(vertex.rgba);
        }


        writer.write_u32(self.indices.len() as _);
        for index in &self.indices {
            writer.write_u32(*index);
        }

        writer.finish()
    }


    pub fn decode(data: &[u8]) -> Result<VoxelMesh, VoxelMeshDecodeError> {
        let decode = || {
        let Some(mut reader) = ByteReader::new(data)
        else {
            return Some(Err(VoxelMeshDecodeError::InvalidByteWriter));
        };

        let magic = reader.read()?;

        if magic != VOXEL_MESH_MAGIC {
            return Some(Err(VoxelMeshDecodeError::InvalidMagicValue));
        }

        let version = reader.read()?;
        if version != VOXEL_MESH_VERSION {
            return Some(Err(VoxelMeshDecodeError::InvalidVersion {
                lib_version: VOXEL_MESH_VERSION,
                file_version: version,
            }));
        }


        let vertices_len = reader.read_u32()?;
        let mut vertices = Vec::with_capacity(vertices_len as _);

        for _ in 0..vertices_len {
            let x = reader.read_f32()?;
            let y = reader.read_f32()?;
            let z = reader.read_f32()?;
            let pos = Vec3::new(x, y, z);

            let rgba = reader.read_u32()?;

            vertices.push(Vertex::new(pos, rgba));
        }

        let indices_len = reader.read_u32()?;
        let mut indices = Vec::with_capacity(vertices_len as _);
        for _ in 0..indices_len {
            indices.push(reader.read_u32()?);
        }

        Some(Ok(Self { vertices, indices }))
        };


        decode().unwrap_or(Err(VoxelMeshDecodeError::EOI))
    }
}


impl Vertex {
    pub fn new(position: Vec3, colour: u32) -> Self {
        Self { position, rgba: colour }
    }
}


pub fn greedy_mesh(rgba_data: &[u32], dims: USizeVec3, vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>) -> bool {
    assert_eq!(rgba_data.len(), dims.x * dims.y * dims.z);
    let idims = dims.as_ivec3();
    // sweep over each axis
    
    let block_size = (1.0 / dims.as_vec3()).with_z(0.1);

    for d in 0..3 {
        let u = (d + 1) % 3;
        let v = (d + 2) % 3;
        let mut x = IVec3::ZERO;

        let mut block_mask = vec![(0u32, false); dims.x * dims.y * dims.z];

       
        x[d] = -1;
        while x[d] < idims[d] {
            let mut n = 0;
            x[v] = 0;

            while x[v] < idims[v] {
                x[u] = 0;

                while x[u] < idims[u] {

                    let block_current = {
                        let r = x;
                        let is_out_of_bounds =    r.x < 0
                                               || r.y < 0
                                               || r.z < 0;

                        if is_out_of_bounds {
                            0
                        } else {
                            rgba_data[(r.z * idims.y * idims.x + r.y * idims.x + r.x) as usize]
                        }
                    };

                    let block_compare = {
                        let mut r = x;
                        r[d] += 1;
                        let is_out_of_bounds =    r.x == idims.x
                                               || r.y == idims.y
                                               || r.z == idims.z;

                        if is_out_of_bounds {
                            0
                        } else {
                            rgba_data[(r.z * idims.y * idims.x + r.y * idims.x + r.x) as usize]
                        }
                    };

                    // the mask is set to true if there is a visible face
                    // between two blocks, i.e. both aren't empty and both aren't blocks
                    block_mask[n] = match (block_current == 0, block_compare == 0) {
                        (true, false) => (block_compare, true),
                        (false, true) => (block_current, false),
                        (_, _) => (0, false),
                    };

                    n += 1;
                    x[u] += 1;
                }

                x[v] += 1;
            }


            x[d] += 1;


            let mut n = 0;
            for j in 0..dims.x {
                let mut i = 0;
                while i < dims.y {
                    if block_mask[n].0 == 0 {
                        i += 1;
                        n += 1;
                        continue;
                    }

                    let (kind, neg_d) = block_mask[n];

                    
                    // Compute the width of this quad and store it in w                        
                    //   This is done by searching along the current axis until mask[n + w] is false
                    let mut w = 1;
                    while i + w < dims.x && block_mask[n + w] == (kind, neg_d) { w += 1; }


                    // Compute the height of this quad and store it in h                        
                    //   This is done by checking if every block next to this row (range 0 to w) is also part of the mask.
                    //   For example, if w is 5 we currently have a quad of dimensions 1 x 5. To reduce triangle count,
                    //   greedy meshing will attempt to expand this quad out to CHUNK_SIZE x 5, but will stop if it reaches a hole in the mask
                    
                    let mut done = false;
                    let mut h = 1;
                    while j + h < dims.y {
                        for k in 0..w {
                            // if there's a hole in the mask, exit
                            if block_mask[n + k + h * dims.x] != (kind, neg_d) {
                                done = true;
                                break;
                            }
                        }


                        if done { break }

                        h += 1;
                    }


                    x[u] = i as _;
                    x[v] = j as _;

                    // du and dv determine the size and orientation of this face
                    let mut du = IVec3::ZERO;
                    du[u] = w as _;

                    let mut dv = IVec3::ZERO;
                    dv[v] = h as _;


                    let mut x = x.as_vec3();
                    let mut du = du.as_vec3();
                    let mut dv = dv.as_vec3();
                    if d == 1 {
                        x = Vec3::new(x.z, x.y, x.x);
                        du = Vec3::new(du.z, du.y, du.x);
                        dv = Vec3::new(dv.z, dv.y, dv.x);
                    }

                    x -= dims.as_vec3() * 0.5;
                    x = x * block_size;
                    du = du * block_size;
                    dv = dv * block_size;


                    let quad = Quad {
                                colour: kind,
                                corners: if !neg_d {[
                                    x,
                                    (x+du),
                                    (x+du+dv),
                                    (x+dv),
                                ]} else {[
                                    (x+dv),
                                    (x+du+dv),
                                    (x+du),
                                    x,
                                ]
                                },
                            };

                    draw_quad(vertices, indices, quad);


                    // clear this part of the mask so we don't add duplicates
                    for h in 0..h  {
                        for w in 0..w {
                            block_mask[n+w+h*dims.x].0 = 0;
                        }
                    }

                    // increment counters and continue
                    i += w;
                    n += w;
                }
            }

        }
    }
    true
}

