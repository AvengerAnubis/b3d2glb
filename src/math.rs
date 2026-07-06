/// Row-major 4×4 matrix: `m[row][col]`.
pub type Mat4 = [[f32; 4]; 4];

/// Convert a B3D position `[x, y, z]` (left-handed Y-up) to right-handed Y-up
/// by negating Z.
pub fn neg_z_pos(p: [f32; 3]) -> [f32; 3] {
    [p[0], p[1], -p[2]]
}

/// Convert a B3D quaternion `[w, x, y, z]` (left-handed Y-up) to right-handed
/// Y-up by negating the Z component of the rotation axis.
/// Result is still `[w, x, y, z]`; call sites reorder to glTF's `[x, y, z, w]`.
pub fn neg_z_quat(q: [f32; 4]) -> [f32; 4] {
    [q[0], q[1], q[2], -q[3]]
}

/// Build a row-major TRS matrix from B3D bind-pose data.
///
/// `pos` and `rot` should already be converted to right-handed Y-up
/// (via `neg_z_pos` / `neg_z_quat`).
///
/// Row-major convention: `m[row][col]`, translation in `m[3][0..2]`.
pub fn b3d_to_mat4(pos: [f32; 3], scale: [f32; 3], rot: [f32; 4]) -> Mat4 {
    let (x, y, z, w) = (rot[1], rot[2], rot[3], rot[0]);
    let xx = x * x; let yy = y * y; let zz = z * z;
    let xy = x * y; let xz = x * z; let yz = y * z;
    let wx = w * x; let wy = w * y; let wz = w * z;

    let mut m = [[0.0f32; 4]; 4];

    m[0][0] = (1.0 - 2.0 * (yy + zz)) * scale[0];
    m[0][1] = 2.0 * (xy + wz) * scale[1];
    m[0][2] = 2.0 * (xz - wy) * scale[2];
    m[0][3] = 0.0;

    m[1][0] = 2.0 * (xy - wz) * scale[0];
    m[1][1] = (1.0 - 2.0 * (xx + zz)) * scale[1];
    m[1][2] = 2.0 * (yz + wx) * scale[2];
    m[1][3] = 0.0;

    m[2][0] = 2.0 * (xz + wy) * scale[0];
    m[2][1] = 2.0 * (yz - wx) * scale[1];
    m[2][2] = (1.0 - 2.0 * (xx + yy)) * scale[2];
    m[2][3] = 0.0;

    m[3][0] = pos[0];
    m[3][1] = pos[1];
    m[3][2] = pos[2];
    m[3][3] = 1.0;

    m
}

/// Row-major 4×4 matrix multiply: `r = a * b`.
pub fn mat4_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut r = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            r[i][j] = a[i][0] * b[0][j]
                     + a[i][1] * b[1][j]
                     + a[i][2] * b[2][j]
                     + a[i][3] * b[3][j];
        }
    }
    r
}

/// Inverse of a row-major 4×4 matrix using cofactors.
pub fn mat4_inverse(m: &Mat4) -> Mat4 {
    let (m00, m01, m02, m03) = (m[0][0], m[0][1], m[0][2], m[0][3]);
    let (m10, m11, m12, m13) = (m[1][0], m[1][1], m[1][2], m[1][3]);
    let (m20, m21, m22, m23) = (m[2][0], m[2][1], m[2][2], m[2][3]);
    let (m30, m31, m32, m33) = (m[3][0], m[3][1], m[3][2], m[3][3]);

    let a = m00*m11 - m01*m10; let b = m00*m12 - m02*m10;
    let c = m00*m13 - m03*m10; let d = m01*m12 - m02*m11;
    let e = m01*m13 - m03*m11; let f = m02*m13 - m03*m12;
    let g = m20*m31 - m21*m30; let h = m20*m32 - m22*m30;
    let i = m20*m33 - m23*m30; let j = m21*m32 - m22*m31;
    let k = m21*m33 - m23*m31; let l = m22*m33 - m23*m32;

    let det = a*l - b*k + c*j + d*i - e*h + f*g;
    if det == 0.0 { return [[0.0; 4]; 4]; }
    let inv_det = 1.0 / det;

    let mut inv = [[0.0; 4]; 4];
    inv[0][0] = ( m11*l - m12*k + m13*j) * inv_det;
    inv[0][1] = (-m01*l + m02*k - m03*j) * inv_det;
    inv[0][2] = ( m31*f - m32*e + m33*d) * inv_det;
    inv[0][3] = (-m21*f + m22*e - m23*d) * inv_det;
    inv[1][0] = (-m10*l + m12*i - m13*h) * inv_det;
    inv[1][1] = ( m00*l - m02*i + m03*h) * inv_det;
    inv[1][2] = (-m30*f + m32*c - m33*b) * inv_det;
    inv[1][3] = ( m20*f - m22*c + m23*b) * inv_det;
    inv[2][0] = ( m10*k - m11*i + m13*g) * inv_det;
    inv[2][1] = (-m00*k + m01*i - m03*g) * inv_det;
    inv[2][2] = ( m30*e - m31*c + m33*a) * inv_det;
    inv[2][3] = (-m20*e + m21*c - m23*a) * inv_det;
    inv[3][0] = (-m10*j + m11*h - m12*g) * inv_det;
    inv[3][1] = ( m00*j - m01*h + m02*g) * inv_det;
    inv[3][2] = (-m30*d + m31*b - m32*a) * inv_det;
    inv[3][3] = ( m20*d - m21*b + m22*a) * inv_det;
    inv
}
