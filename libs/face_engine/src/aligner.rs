use crate::types::Point;
use crate::FaceEngineError;
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgb, RgbImage};

const DST_POINTS: [Point; 5] = [
    Point { x: 38.2946, y: 51.6963 },
    Point { x: 73.5318, y: 51.5014 },
    Point { x: 56.0252, y: 71.7366 },
    Point { x: 41.5493, y: 92.3655 },
    Point { x: 70.7299, y: 92.2041 },
];

const OUTPUT_SIZE: u32 = 112;

pub struct FaceAligner;

impl FaceAligner {
    pub fn align(image_bytes: &[u8], landmarks: &[f32; 10]) -> Result<DynamicImage, FaceEngineError> {
        let img = image::load_from_memory(image_bytes)?;

        let src_points = [
            Point { x: landmarks[0], y: landmarks[1] },
            Point { x: landmarks[2], y: landmarks[3] },
            Point { x: landmarks[4], y: landmarks[5] },
            Point { x: landmarks[6], y: landmarks[7] },
            Point { x: landmarks[8], y: landmarks[9] },
        ];

        let transform = Self::estimate_similarity_transform(&src_points, &DST_POINTS)?;

        let aligned = Self::apply_transform(&img, &transform, OUTPUT_SIZE, OUTPUT_SIZE);

        Ok(DynamicImage::ImageRgb8(aligned))
    }

    pub fn align_from_image(
        img: &DynamicImage,
        landmarks: &[f32; 10],
    ) -> Result<DynamicImage, FaceEngineError> {
        let src_points = [
            Point { x: landmarks[0], y: landmarks[1] },
            Point { x: landmarks[2], y: landmarks[3] },
            Point { x: landmarks[4], y: landmarks[5] },
            Point { x: landmarks[6], y: landmarks[7] },
            Point { x: landmarks[8], y: landmarks[9] },
        ];

        let transform = Self::estimate_similarity_transform(&src_points, &DST_POINTS)?;

        let aligned = Self::apply_transform(img, &transform, OUTPUT_SIZE, OUTPUT_SIZE);

        Ok(DynamicImage::ImageRgb8(aligned))
    }

    fn estimate_similarity_transform(
        src_points: &[Point; 5],
        dst_points: &[Point; 5],
    ) -> Result<[f32; 6], FaceEngineError> {
        let n = 5usize;

        let mut sum_x = 0.0f64;
        let mut sum_y = 0.0f64;
        let mut sum_u = 0.0f64;
        let mut sum_v = 0.0f64;
        let mut sum_xx = 0.0f64;
        let mut sum_yy = 0.0f64;
        let mut sum_ux = 0.0f64;
        let mut sum_uy = 0.0f64;
        let mut sum_vx = 0.0f64;
        let mut sum_vy = 0.0f64;

        for i in 0..n {
            let x = src_points[i].x as f64;
            let y = src_points[i].y as f64;
            let u = dst_points[i].x as f64;
            let v = dst_points[i].y as f64;

            sum_x += x;
            sum_y += y;
            sum_u += u;
            sum_v += v;
            sum_xx += x * x;
            sum_yy += y * y;
            sum_ux += u * x;
            sum_uy += u * y;
            sum_vx += v * x;
            sum_vy += v * y;
        }

        let n_f = n as f64;
        let det = n_f * (sum_xx + sum_yy) - sum_x * sum_x - sum_y * sum_y;

        if det.abs() < 1e-10 {
            return Err(FaceEngineError::AlignmentFailed);
        }

        let a = (n_f * sum_ux - sum_u * sum_x + n_f * sum_vy - sum_v * sum_y) / det;
        let b = (n_f * sum_uy - sum_u * sum_y - n_f * sum_vx + sum_v * sum_x) / det;
        let c = (sum_u - a * sum_x - b * sum_y) / n_f;
        let d = (sum_v + b * sum_x - a * sum_y) / n_f;

        Ok([a as f32, b as f32, c as f32, -b as f32, a as f32, d as f32])
    }

    fn apply_transform(
        img: &DynamicImage,
        transform: &[f32; 6],
        width: u32,
        height: u32,
    ) -> RgbImage {
        let mut result: RgbImage = ImageBuffer::new(width, height);

        let a = transform[0];
        let b = transform[1];
        let c = transform[2];
        let d = transform[3];
        let e = transform[4];
        let f = transform[5];

        let det = a * e - b * d;
        let inv_det = if det.abs() > 1e-10 { 1.0 / det } else { 0.0 };

        for y in 0..height {
            for x in 0..width {
                let x_f = x as f32;
                let y_f = y as f32;

                let src_x = e * inv_det * (x_f - c) - b * inv_det * (y_f - f);
                let src_y = -d * inv_det * (x_f - c) + a * inv_det * (y_f - f);

                if src_x >= 0.0 && src_x < img.width() as f32
                    && src_y >= 0.0 && src_y < img.height() as f32
                {
                    let pixel = Self::bilinear_interpolate(img, src_x, src_y);
                    result.put_pixel(x, y, pixel);
                } else {
                    result.put_pixel(x, y, Rgb([0, 0, 0]));
                }
            }
        }

        result
    }

    fn bilinear_interpolate(img: &DynamicImage, x: f32, y: f32) -> Rgb<u8> {
        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(img.width() - 1);
        let y1 = (y0 + 1).min(img.height() - 1);

        let dx = x - x0 as f32;
        let dy = y - y0 as f32;

        let p00 = img.get_pixel(x0, y0);
        let p01 = img.get_pixel(x1, y0);
        let p10 = img.get_pixel(x0, y1);
        let p11 = img.get_pixel(x1, y1);

        let r = Self::lerp(
            Self::lerp(p00[0] as f32, p01[0] as f32, dx),
            Self::lerp(p10[0] as f32, p11[0] as f32, dx),
            dy,
        ) as u8;
        let g = Self::lerp(
            Self::lerp(p00[1] as f32, p01[1] as f32, dx),
            Self::lerp(p10[1] as f32, p11[1] as f32, dx),
            dy,
        ) as u8;
        let b = Self::lerp(
            Self::lerp(p00[2] as f32, p01[2] as f32, dx),
            Self::lerp(p10[2] as f32, p11[2] as f32, dx),
            dy,
        ) as u8;

        Rgb([r, g, b])
    }

    fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }
}
