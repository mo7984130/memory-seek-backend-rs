use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl BBox {
    /// 计算边界框的面积
    pub fn area(&self) -> f32 {
        self.w * self.h
    }

    /// 计算与另一个边界框的交并比 (Intersection over Union)
    ///
    /// # 参数
    /// - `other`: 另一个边界框
    pub fn iou(&self, other: &BBox) -> f32 {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.w).min(other.x + other.w);
        let y2 = (self.y + self.h).min(other.y + other.h);

        let intersection = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
        let union = self.area() + other.area() - intersection;

        if union > 0.0 {
            intersection / union
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone)]
pub struct FaceDetection {
    pub bbox: BBox,
    pub landmarks: [f32; 10],
    pub score: f32,
}

impl FaceDetection {
    /// 创建人脸检测结果
    ///
    /// # 参数
    /// - `bbox`: 边界框
    /// - `landmarks`: 5 个关键点的坐标，共 10 个浮点值
    /// - `score`: 检测置信度
    pub fn new(bbox: BBox, landmarks: [f32; 10], score: f32) -> Self {
        Self { bbox, landmarks, score }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    /// 创建二维坐标点
    ///
    /// # 参数
    /// - `x`: 横坐标
    /// - `y`: 纵坐标
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}
