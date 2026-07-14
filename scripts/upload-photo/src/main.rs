use std::env;

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use entities::photo::photo::{Entity, Model};
use rexif::{ExifData, ExifTag};
use s3::{Bucket, Region, creds::Credentials};
use sea_orm::{
    ActiveModelTrait, EntityTrait, IntoActiveModel, Set,
};

#[tokio::main]
async fn main() {
    // 初始化S3配置
    let region = Region::Custom {
        region: env::var("REGION").expect("REGION not set"),
        endpoint: env::var("ENDPOINT").expect("ENDPOINT not set"),
    };

    let access_key = env::var("ACCESS_KEY").expect("ACCESS_KEY not set");
    let secret_key = env::var("SECRET_KEY").expect("SECRET_KEY not set");

    let credentials = Credentials::new(
        Some(access_key.as_str()),
        Some(secret_key.as_str()),
        None,
        None,
        None,
    )
    .expect("Failed to create S3 credentials");

    let bucket = Bucket::new(
        &env::var("BUCKET").expect("BUCKET not set"),
        region,
        credentials,
    )
    .expect("Failed to create S3 bucket");

    // 连接数据库
    let db = sea_orm::Database::connect(env::var("DB_URL").expect("DB_URL not set"))
        .await
        .expect("Failed to connect to database");

    // 查询所有照片
    let photos = Entity::find()
        .all(&db)
        .await
        .expect("Failed to query photos");

    let total = photos.len();

    for (i, photo) in photos.iter().enumerate() {
        print!("{}/{}: {} ", i + 1, total, photo.file_id);

        // 从S3获取文件前16KB用于读取EXIF
        let bytes = match bucket
            .get_object_range(photo.file_id.clone(), 0, Some(1024 * 256))
            .await
        {
            Ok(response) => response.bytes().to_vec(),
            Err(e) => {
                println!("- S3获取失败: {}", e);
                continue;
            }
        };

        // 解析EXIF数据
        let exif_data = match rexif::parse_buffer(&bytes) {
            Ok(data) => Some(data),
            Err(e) => {
                match e {
                    rexif::ExifError::ExifIfdEntryNotFound => {
                        // 没有EXIF数据，使用降级方案
                        None
                    }
                    _ => {
                        println!("- EXIF解析失败: {}", e);
                        None
                    }
                }
            }
        };

        // 尝试从EXIF获取拍摄日期
        let mut photo_date = None;

        if let Some(ref exif) = exif_data {
            photo_date = extract_date_from_exif(exif);
            if let Some(date) = photo_date {
                // 使用安全的日期创建方式
                if let Some(min_valid_date) =
                    NaiveDate::from_ymd_opt(2005, 1, 1).and_then(|d| d.and_hms_opt(0, 0, 0))
                {
                    if date < min_valid_date {
                        photo_date = None;
                    }
                }
            }
        }

        // 降级方案：从file_id提取日期
        if photo_date.is_none() {
            photo_date = extract_date_from_file_id(&photo.file_id);
        }

        // 更新数据库
        match photo_date {
            Some(date) => {
                println!("- 日期: {}", date);
                update_photo_date(&db, photo, date).await;
            }
            None => {
                println!("- 无法提取日期");
            }
        }
    }

    println!("\n处理完成！");
}

/// 从EXIF数据中提取拍摄日期
fn extract_date_from_exif(exif: &ExifData) -> Option<NaiveDateTime> {
    // 尝试多个可能的日期标签
    let date_tags = [
        ExifTag::DateTimeOriginal,
        ExifTag::DateTimeDigitized,
        ExifTag::DateTime,
    ];

    for tag in &date_tags {
        if let Some(entry) = exif.entries.iter().find(|e| e.tag == *tag) {
            let date_str = entry.value_more_readable.to_string();

            // 尝试多种日期格式
            if let Ok(date) = NaiveDateTime::parse_from_str(&date_str, "%Y:%m:%d %H:%M:%S") {
                return Some(date);
            }

            // 某些相机使用其他格式
            if let Ok(date) = NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S") {
                return Some(date);
            }
        }
    }

    None
}

/// 从file_id路径中提取日期
/// 支持的格式: photos/2026/02/03/filename.jpg
fn extract_date_from_file_id(file_id: &str) -> Option<NaiveDateTime> {
    let parts: Vec<&str> = file_id.split('/').collect();

    if parts.len() >= 4 {
        // 尝试解析年份、月份、日期
        if let (Ok(year), Ok(month), Ok(day)) = (
            parts[parts.len() - 4].parse::<i32>(),
            parts[parts.len() - 3].parse::<u32>(),
            parts[parts.len() - 2].parse::<u32>(),
        ) {
            // 使用当天的00:00:00作为时间
            return NaiveDateTime::parse_from_str(
                &format!("{}-{:02}-{:02} 00:00:00", year, month, day),
                "%Y-%m-%d %H:%M:%S",
            )
            .ok();
        }
    }

    None
}

/// 更新照片的创建日期
async fn update_photo_date(db: &sea_orm::DatabaseConnection, photo: &Model, date: NaiveDateTime) {
    let local_date = Local.from_local_datetime(&date).unwrap();
    let utc_date: DateTime<Utc> = local_date.with_timezone(&Utc);

    let mut active_model = photo.clone().into_active_model();
    active_model.created_at = Set(utc_date);

    match active_model.update(db).await {
        Ok(_) => (),
        Err(e) => eprintln!("更新失败: {} - {}", photo.file_id, e),
    }
}
