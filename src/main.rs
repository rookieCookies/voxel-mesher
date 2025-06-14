use std::{env, fs, io::BufRead};

use glam::{IVec3, UVec3, Vec3Swizzles};
use voxel_mesher::{greedy_mesh, Voxel, VoxelMesh};

fn main() {
    let mut args = env::args();
    let _ = args.next(); // path

    let Some(data_path) = args.next()
    else {
        println!("[!] missing input file. usage: voxel-mesher [input] [output]");
        return;
    };

    let Some(output_path) = args.next()
    else {
        println!("[!] missing output path. usage: voxel-mesher [input] [output]");
        return;
    };

    let scale = args.next().unwrap_or("32".to_string());

    let Ok(data) = fs::read(&data_path)
    else {
        println!("[!] unable to read the data file at '{data_path}'");
        return;
    };


    let mut voxels = Vec::new();
    let mut mins = IVec3::MAX;
    let mut maxs = IVec3::MIN;

    for (index, line) in data.lines().enumerate() {
        let Ok(line) = line
        else {
            println!("[!] failed to read line {index} on '{data_path}'");
            return;
        };

        // comment
        if line.is_empty() { continue }
        if line.starts_with('#') { continue };

        let mut split = line.split_whitespace();
        let mut successful = false;
        let mut pos = IVec3::MIN;
        let mut colour = 0;

        'parse: {
            let Some(x) : Option<i32> = split.next().map(|v| v.parse().ok()).flatten()
            else { break 'parse };
            let Some(y) : Option<i32> = split.next().map(|v| v.parse().ok()).flatten()
            else { break 'parse };
            let Some(z) : Option<i32> = split.next().map(|v| v.parse().ok()).flatten()
            else { break 'parse };
            let Some(rgb) : Option<u32> = split.next().map(|v| u32::from_str_radix(v, 16).ok()).flatten()
            else { break 'parse };
            let rgba = (rgb << 8) | 0xFF;

            pos = IVec3::new(x, z, y);
            colour = rgba;
            successful = true;
        }

        if !successful {
            println!("[!] invalid syntax on line {index}, found '{line}' expected '[x] [y] [z] [hex]'");
            return;
        }

        mins = mins.min(pos);
        maxs = maxs.max(pos);

        voxels.push(Voxel { pos, colour });
    }


    let dims = (maxs - mins).abs() + 1;
    let mut colours = vec![0; (dims.z * dims.x * dims.y) as usize];

    for voxel in voxels {
        let pos = voxel.pos - mins;
        colours[(pos.z * dims.y * dims.x + pos.y * dims.x + pos.x) as usize] = voxel.colour;
    }


    let mut vertices = vec![];
    let mut indices = vec![];

    let scale = match scale.as_str() {
        "unit" => 1.0 / dims.as_vec3(),
        _ => {
            let scale = scale.parse::<u32>().unwrap();
            1.0 / UVec3::splat(scale).as_vec3()
        }
    };

    greedy_mesh(&colours, dims, &mut vertices, &mut indices, scale);

    let mesh = VoxelMesh { vertices, indices };
    let file = mesh.encode();

    #[cfg(debug_assertions)]
    {
        let decoded_mesh = VoxelMesh::decode(&file).unwrap();
        assert_eq!(mesh, decoded_mesh)
    }


    if let Err(e) = fs::write(output_path, file) {
        println!("[!] failed to write to the output path. {e}");
    }
}





