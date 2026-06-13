# Model Naming Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify model naming conventions across domains/auth, domains/user, and domains/photo modules to use `Param` and `Result` suffixes consistently.

**Architecture:** Direct rename approach - rename all model types following the convention: `Param` for input parameters (Request/Query/Param) and `Result` for output objects (Response/VO/DTO). Special case: `Row` suffix for database query results with `FromQueryResult`. Incremental implementation with compile checks after each module.

**Tech Stack:** Rust, SeaORM, Axum, serde, validator

---

## File Structure

### Files to Modify

**Auth Module (3 files):**
- `domains/auth/src/models/mod.rs` - Model definitions
- `domains/auth/src/services/auth_service.rs` - Service layer
- `domains/auth/src/controller/auth_controller.rs` - Controller layer

**User Module (3 files):**
- `domains/user/src/models/mod.rs` - Model definitions
- `domains/user/src/services/user_service.rs` - Service layer
- `domains/user/src/controller/user_controller.rs` - Controller layer

**Photo Module (11 files):**
- `domains/photo/src/models/photo.rs` - Photo model definitions
- `domains/photo/src/models/comment.rs` - Comment model definitions
- `domains/photo/src/models/collection.rs` - Collection model definitions
- `domains/photo/src/services/photo_service.rs` - Photo service
- `domains/photo/src/services/collection_service.rs` - Collection service
- `domains/photo/src/services/collection_photo_service.rs` - Collection photo service
- `domains/photo/src/services/comment_service.rs` - Comment service
- `domains/photo/src/controllers/photo_controller.rs` - Photo controller
- `domains/photo/src/controllers/collection_controller.rs` - Collection controller
- `domains/photo/src/controllers/collection_photo_controller.rs` - Collection photo controller
- `domains/photo/src/controllers/comment_controller.rs` - Comment controller

---

## Task 1: Rename Auth Module Models

**Files:**
- Modify: `domains/auth/src/models/mod.rs`
- Modify: `domains/auth/src/services/auth_service.rs`
- Modify: `domains/auth/src/controller/auth_controller.rs`

- [ ] **Step 1: Rename LoginRequest to LoginParam in models**

In `domains/auth/src/models/mod.rs`, change:
```rust
// Before
pub struct LoginRequest {

// After
pub struct LoginParam {
```

- [ ] **Step 2: Rename RegisterRequest to RegisterParam in models**

In `domains/auth/src/models/mod.rs`, change:
```rust
// Before
pub struct RegisterRequest {

// After
pub struct RegisterParam {
```

- [ ] **Step 3: Rename SendEmailCodeRequest to SendEmailCodeParam in models**

In `domains/auth/src/models/mod.rs`, change:
```rust
// Before
pub struct SendEmailCodeRequest {

// After
pub struct SendEmailCodeParam {
```

- [ ] **Step 4: Rename AccessTokenResponse to AccessTokenResult in models**

In `domains/auth/src/models/mod.rs`, change:
```rust
// Before
pub struct AccessTokenResponse {

// After
pub struct AccessTokenResult {
```

- [ ] **Step 5: Update imports in auth_service.rs**

In `domains/auth/src/services/auth_service.rs`, change line 3:
```rust
// Before
use crate::models::{AccessTokenResponse, LoginRequest, RegisterRequest, SendEmailCodeRequest};

// After
use crate::models::{AccessTokenResult, LoginParam, RegisterParam, SendEmailCodeParam};
```

- [ ] **Step 6: Update function signature for login in auth_service.rs**

In `domains/auth/src/services/auth_service.rs`, change line 52:
```rust
// Before
pub async fn login(state: &AuthState, req: LoginRequest) -> Result<UserDTO, AppError> {

// After
pub async fn login(state: &AuthState, req: LoginParam) -> Result<UserDTO, AppError> {
```

- [ ] **Step 7: Update function signature for register in auth_service.rs**

In `domains/auth/src/services/auth_service.rs`, change line 240:
```rust
// Before
pub async fn register(state: &AuthState, req: RegisterRequest) -> Result<UserDTO, AppError> {

// After
pub async fn register(state: &AuthState, req: RegisterParam) -> Result<UserDTO, AppError> {
```

- [ ] **Step 8: Update function signature for send_email_code in auth_service.rs**

In `domains/auth/src/services/auth_service.rs`, change line 337:
```rust
// Before
pub async fn send_email_code(state: &AuthState, req: SendEmailCodeRequest) -> Result<(), AppError> {

// After
pub async fn send_email_code(state: &AuthState, req: SendEmailCodeParam) -> Result<(), AppError> {
```

- [ ] **Step 9: Update function signature for refresh_access_token in auth_service.rs**

In `domains/auth/src/services/auth_service.rs`, change line 402:
```rust
// Before
) -> Result<AccessTokenResponse, AppError> {

// After
) -> Result<AccessTokenResult, AppError> {
```

- [ ] **Step 10: Update struct initialization in refresh_access_token**

In `domains/auth/src/services/auth_service.rs`, change line 426:
```rust
// Before
    Ok(AccessTokenResponse {

// After
    Ok(AccessTokenResult {
```

- [ ] **Step 11: Update imports in auth_controller.rs**

In `domains/auth/src/controller/auth_controller.rs`, change lines 2-3:
```rust
// Before
use crate::models::SendEmailCodeRequest;
use crate::models::{AccessTokenResponse, LoginRequest, RegisterRequest};

// After
use crate::models::SendEmailCodeParam;
use crate::models::{AccessTokenResult, LoginParam, RegisterParam};
```

- [ ] **Step 12: Update ValidatedJson type in login handler**

In `domains/auth/src/controller/auth_controller.rs`, change line 50:
```rust
// Before
        ValidatedJson(req): ValidatedJson<LoginRequest>,

// After
        ValidatedJson(req): ValidatedJson<LoginParam>,
```

- [ ] **Step 13: Update ValidatedJson type in register handler**

In `domains/auth/src/controller/auth_controller.rs`, change line 69:
```rust
// Before
        ValidatedJson(payload): ValidatedJson<RegisterRequest>,

// After
        ValidatedJson(payload): ValidatedJson<RegisterParam>,
```

- [ ] **Step 14: Update ValidatedJson type in send_email_code handler**

In `domains/auth/src/controller/auth_controller.rs`, change line 87:
```rust
// Before
        ValidatedJson(payload): ValidatedJson<SendEmailCodeRequest>,

// After
        ValidatedJson(payload): ValidatedJson<SendEmailCodeParam>,
```

- [ ] **Step 15: Update return type in refresh_access_token handler**

In `domains/auth/src/controller/auth_controller.rs`, change line 111:
```rust
// Before
    ) -> Result<R<AccessTokenResponse>, AppError> {

// After
    ) -> Result<R<AccessTokenResult>, AppError> {
```

- [ ] **Step 16: Compile check auth module**

Run: `cargo check --features auth`
Expected: No compilation errors

- [ ] **Step 17: Commit auth module changes**

```bash
git add domains/auth/src/models/mod.rs domains/auth/src/services/auth_service.rs domains/auth/src/controller/auth_controller.rs
git commit -m "refactor(auth): rename model types to Param/Result convention

- LoginRequest → LoginParam
- RegisterRequest → RegisterParam
- SendEmailCodeRequest → SendEmailCodeParam
- AccessTokenResponse → AccessTokenResult"
```

---

## Task 2: Rename User Module Models

**Files:**
- Modify: `domains/user/src/models/mod.rs`
- Modify: `domains/user/src/services/user_service.rs`
- Modify: `domains/user/src/controller/user_controller.rs`

- [ ] **Step 1: Rename ChangePasswordRequest to ChangePasswordParam in models**

In `domains/user/src/models/mod.rs`, change:
```rust
// Before
pub struct ChangePasswordRequest {

// After
pub struct ChangePasswordParam {
```

- [ ] **Step 2: Rename ChangeNicknameRequest to ChangeNicknameParam in models**

In `domains/user/src/models/mod.rs`, change:
```rust
// Before
pub struct ChangeNicknameRequest {

// After
pub struct ChangeNicknameParam {
```

- [ ] **Step 3: Rename GetUserInfoBatchRequest to GetUserInfoBatchParam in models**

In `domains/user/src/models/mod.rs`, change:
```rust
// Before
pub struct GetUserInfoBatchRequest {

// After
pub struct GetUserInfoBatchParam {
```

- [ ] **Step 4: Rename InviterCodeDTO to InviterCodeResult in models**

In `domains/user/src/models/mod.rs`, change:
```rust
// Before
pub struct InviterCodeDTO {

// After
pub struct InviterCodeResult {
```

- [ ] **Step 5: Rename UserInfoDTO to UserInfoRow in models**

In `domains/user/src/models/mod.rs`, change:
```rust
// Before
pub struct UserInfoDTO {

// After
pub struct UserInfoRow {
```

- [ ] **Step 6: Rename UserInfoVO to UserInfoResult in models**

In `domains/user/src/models/mod.rs`, change:
```rust
// Before
pub struct UserInfoVO {

// After
pub struct UserInfoResult {
```

- [ ] **Step 7: Update impl block and from_dto method**

In `domains/user/src/models/mod.rs`, change:
```rust
// Before
impl UserInfoVO {
    pub fn from_dto(dto: UserInfoDTO, token_cipher: &TokenCipher) -> Self {

// After
impl UserInfoResult {
    pub fn from_dto(dto: UserInfoRow, token_cipher: &TokenCipher) -> Self {
```

- [ ] **Step 8: Update test function names and types**

In `domains/user/src/models/mod.rs`, update all test functions to use new type names:
```rust
// Before
fn test_user_info_vo_from_dto_with_avatar() {
    let dto = UserInfoDTO {
    let vo = UserInfoVO::from_dto(dto, &cipher);

// After
fn test_user_info_vo_from_dto_with_avatar() {
    let dto = UserInfoRow {
    let vo = UserInfoResult::from_dto(dto, &cipher);
```

Apply similar changes to `test_user_info_vo_from_dto_without_avatar`.

- [ ] **Step 9: Update test function names for ChangeNicknameRequest**

In `domains/user/src/models/mod.rs`, update:
```rust
// Before
fn test_change_nickname_request_valid() {
    let req = ChangeNicknameRequest {

// After
fn test_change_nickname_request_valid() {
    let req = ChangeNicknameParam {
```

Apply similar changes to all `test_change_nickname_request_*` and `test_change_password_request_*` functions.

- [ ] **Step 10: Update imports in user_service.rs**

In `domains/user/src/services/user_service.rs`, change line 15:
```rust
// Before
use crate::models::{ChangePasswordRequest, InviterCodeDTO, UserInfoDTO, UserInfoVO};

// After
use crate::models::{ChangePasswordParam, InviterCodeResult, UserInfoRow, UserInfoResult};
```

- [ ] **Step 11: Update function signature for generate_inviter_code**

In `domains/user/src/services/user_service.rs`, change line 87:
```rust
// Before
) -> Result<InviterCodeDTO, AppError> {

// After
) -> Result<InviterCodeResult, AppError> {
```

- [ ] **Step 12: Update struct initialization in generate_inviter_code**

In `domains/user/src/services/user_service.rs`, change line 117:
```rust
// Before
            return Ok(InviterCodeDTO {

// After
            return Ok(InviterCodeResult {
```

- [ ] **Step 13: Update function signature for change_password**

In `domains/user/src/services/user_service.rs`, change line 331:
```rust
// Before
    req: ChangePasswordRequest,

// After
    req: ChangePasswordParam,
```

- [ ] **Step 14: Update function signature for get_user_info_batch**

In `domains/user/src/services/user_service.rs`, change line 468:
```rust
// Before
) -> Result<Vec<Option<UserInfoVO>>, AppError> {

// After
) -> Result<Vec<Option<UserInfoResult>>, AppError> {
```

- [ ] **Step 15: Update type annotation in get_user_info_batch**

In `domains/user/src/services/user_service.rs`, change line 482:
```rust
// Before
    let result: Vec<Option<UserInfoDTO>> = state

// After
    let result: Vec<Option<UserInfoRow>> = state
```

- [ ] **Step 16: Update into_model call in get_user_info_batch**

In `domains/user/src/services/user_service.rs`, change line 497:
```rust
// Before
                        .into_model::<UserInfoDTO>()

// After
                        .into_model::<UserInfoRow>()
```

- [ ] **Step 17: Update map closure in get_user_info_batch**

In `domains/user/src/services/user_service.rs`, change line 514:
```rust
// Before
        .map(|opt| opt.map(|dto| UserInfoVO::from_dto(dto, &state.token_cipher)))

// After
        .map(|opt| opt.map(|dto| UserInfoResult::from_dto(dto, &state.token_cipher)))
```

- [ ] **Step 18: Update imports in user_controller.rs**

In `domains/user/src/controller/user_controller.rs`, change lines 14-17:
```rust
// Before
use crate::models::{
    ChangeNicknameRequest, ChangePasswordRequest, GetUserInfoBatchRequest, InviterCodeDTO,
    UserInfoVO,
};

// After
use crate::models::{
    ChangeNicknameParam, ChangePasswordParam, GetUserInfoBatchParam, InviterCodeResult,
    UserInfoResult,
};
```

- [ ] **Step 19: Update return type in generate_inviter_code handler**

In `domains/user/src/controller/user_controller.rs`, change line 77:
```rust
// Before
    ) -> Result<R<InviterCodeDTO>, AppError> {

// After
    ) -> Result<R<InviterCodeResult>, AppError> {
```

- [ ] **Step 20: Update ValidatedJson type in change_nickname handler**

In `domains/user/src/controller/user_controller.rs`, change line 98:
```rust
// Before
        ValidatedJson(req): ValidatedJson<ChangeNicknameRequest>,

// After
        ValidatedJson(req): ValidatedJson<ChangeNicknameParam>,
```

- [ ] **Step 21: Update ValidatedJson type in change_password handler**

In `domains/user/src/controller/user_controller.rs`, change line 162:
```rust
// Before
        ValidatedJson(req): ValidatedJson<ChangePasswordRequest>,

// After
        ValidatedJson(req): ValidatedJson<ChangePasswordParam>,
```

- [ ] **Step 22: Update ValidatedJson type in get_user_info_batch handler**

In `domains/user/src/controller/user_controller.rs`, change line 200:
```rust
// Before
        ValidatedJson(req): ValidatedJson<GetUserInfoBatchRequest>,

// After
        ValidatedJson(req): ValidatedJson<GetUserInfoBatchParam>,
```

- [ ] **Step 23: Update return type in get_user_info_batch handler**

In `domains/user/src/controller/user_controller.rs`, change line 201:
```rust
// Before
    ) -> Result<R<Vec<Option<UserInfoVO>>>, AppError> {

// After
    ) -> Result<R<Vec<Option<UserInfoResult>>>, AppError> {
```

- [ ] **Step 24: Compile check user module**

Run: `cargo check --features user`
Expected: No compilation errors

- [ ] **Step 25: Commit user module changes**

```bash
git add domains/user/src/models/mod.rs domains/user/src/services/user_service.rs domains/user/src/controller/user_controller.rs
git commit -m "refactor(user): rename model types to Param/Result convention

- ChangePasswordRequest → ChangePasswordParam
- ChangeNicknameRequest → ChangeNicknameParam
- GetUserInfoBatchRequest → GetUserInfoBatchParam
- InviterCodeDTO → InviterCodeResult
- UserInfoDTO → UserInfoRow
- UserInfoVO → UserInfoResult"
```

---

## Task 3: Rename Photo Module Models

**Files:**
- Modify: `domains/photo/src/models/photo.rs`
- Modify: `domains/photo/src/models/comment.rs`
- Modify: `domains/photo/src/models/collection.rs`
- Modify: `domains/photo/src/services/photo_service.rs`
- Modify: `domains/photo/src/services/collection_service.rs`
- Modify: `domains/photo/src/services/collection_photo_service.rs`
- Modify: `domains/photo/src/services/comment_service.rs`
- Modify: `domains/photo/src/controllers/photo_controller.rs`
- Modify: `domains/photo/src/controllers/collection_controller.rs`
- Modify: `domains/photo/src/controllers/collection_photo_controller.rs`
- Modify: `domains/photo/src/controllers/comment_controller.rs`

- [ ] **Step 1: Rename PhotoVO to PhotoResult in photo.rs**

In `domains/photo/src/models/photo.rs`, change:
```rust
// Before
pub struct PhotoVO {

// After
pub struct PhotoResult {
```

- [ ] **Step 2: Update From implementation for PhotoResult**

In `domains/photo/src/models/photo.rs`, change:
```rust
// Before
impl From<PhotoRecord> for PhotoVO {

// After
impl From<PhotoRecord> for PhotoResult {
```

- [ ] **Step 3: Update impl block for PhotoResult**

In `domains/photo/src/models/photo.rs`, change:
```rust
// Before
impl PhotoVO {

// After
impl PhotoResult {
```

- [ ] **Step 4: Rename PhotoCursorQuery to PhotoCursorParam in photo.rs**

In `domains/photo/src/models/photo.rs`, change:
```rust
// Before
pub struct PhotoCursorQuery {

// After
pub struct PhotoCursorParam {
```

- [ ] **Step 5: Update Default implementation for PhotoCursorParam**

In `domains/photo/src/models/photo.rs`, change:
```rust
// Before
impl Default for PhotoCursorQuery {

// After
impl Default for PhotoCursorParam {
```

- [ ] **Step 6: Update test functions in photo.rs**

In `domains/photo/src/models/photo.rs`, update all test functions:
```rust
// Before
fn test_photo_cursor_query_valid() {
    let param = PhotoCursorQuery {

// After
fn test_photo_cursor_query_valid() {
    let param = PhotoCursorParam {
```

Apply similar changes to all `test_photo_cursor_query_*` functions.

- [ ] **Step 7: Rename PhotoCommentVO to PhotoCommentResult in comment.rs**

In `domains/photo/src/models/comment.rs`, change:
```rust
// Before
pub struct PhotoCommentVO {

// After
pub struct PhotoCommentResult {
```

- [ ] **Step 8: Update From implementation for PhotoCommentResult**

In `domains/photo/src/models/comment.rs`, change:
```rust
// Before
impl From<CommentRecord> for PhotoCommentVO {

// After
impl From<CommentRecord> for PhotoCommentResult {
```

- [ ] **Step 9: Update impl block for PhotoCommentResult**

In `domains/photo/src/models/comment.rs`, change:
```rust
// Before
impl PhotoCommentVO {

// After
impl PhotoCommentResult {
```

- [ ] **Step 10: Rename CommentCursorPageQuery to CommentCursorPageParam in comment.rs**

In `domains/photo/src/models/comment.rs`, change:
```rust
// Before
pub struct CommentCursorPageQuery {

// After
pub struct CommentCursorPageParam {
```

- [ ] **Step 11: Update test functions in comment.rs**

In `domains/photo/src/models/comment.rs`, update all test functions:
```rust
// Before
fn test_comment_cursor_page_query_valid() {
    let param = CommentCursorPageQuery {

// After
fn test_comment_cursor_page_query_valid() {
    let param = CommentCursorPageParam {
```

Apply similar changes to all `test_comment_cursor_page_query_*` functions.

- [ ] **Step 12: Rename CollectionVO to CollectionResult in collection.rs**

In `domains/photo/src/models/collection.rs`, change:
```rust
// Before
pub struct CollectionVO {

// After
pub struct CollectionResult {
```

- [ ] **Step 13: Update From implementation for CollectionResult**

In `domains/photo/src/models/collection.rs`, change:
```rust
// Before
impl From<CollectionRecord> for CollectionVO {
    fn from(record: CollectionRecord) -> Self {
        CollectionVO {

// After
impl From<CollectionRecord> for CollectionResult {
    fn from(record: CollectionRecord) -> Self {
        CollectionResult {
```

- [ ] **Step 14: Update impl block for CollectionResult**

In `domains/photo/src/models/collection.rs`, change:
```rust
// Before
impl CollectionVO {

// After
impl CollectionResult {
```

- [ ] **Step 15: Fix typo and rename CollectionCreateParma to CollectionCreateParam**

In `domains/photo/src/models/collection.rs`, change:
```rust
// Before
pub struct CollectionCreateParma {

// After
pub struct CollectionCreateParam {
```

- [ ] **Step 16: Rename CollectionPhotoCursorPageQuery to CollectionPhotoCursorPageParam**

In `domains/photo/src/models/collection.rs`, change:
```rust
// Before
pub struct CollectionPhotoCursorPageQuery {

// After
pub struct CollectionPhotoCursorPageParam {
```

- [ ] **Step 17: Update test functions in collection.rs**

In `domains/photo/src/models/collection.rs`, update all test functions:
```rust
// Before
fn test_collection_create_param_valid() {
    let param = CollectionCreateParma {

// After
fn test_collection_create_param_valid() {
    let param = CollectionCreateParam {
```

Apply similar changes to all test functions using `CollectionCreateParma` and `CollectionPhotoCursorPageQuery`.

- [ ] **Step 18: Update imports in photo_service.rs**

In `domains/photo/src/services/photo_service.rs`, change line 21:
```rust
// Before
    models::photo::{PhotoCursor, PhotoCursorQuery, PhotoVO},

// After
    models::photo::{PhotoCursor, PhotoCursorParam, PhotoResult},
```

- [ ] **Step 19: Update function signature for load_photos_info**

In `domains/photo/src/services/photo_service.rs`, change line 40:
```rust
// Before
    ) -> Result<Vec<PhotoVO>> {

// After
    ) -> Result<Vec<PhotoResult>> {
```

- [ ] **Step 20: Update PhotoResult usage in load_photos_info**

In `domains/photo/src/services/photo_service.rs`, change line 63:
```rust
// Before
                PhotoVO::from(p.clone())

// After
                PhotoResult::from(p.clone())
```

- [ ] **Step 21: Update function signature for get_photo_cursor_page**

In `domains/photo/src/services/photo_service.rs`, change lines 74-75:
```rust
// Before
        query: PhotoCursorQuery,
    ) -> Result<CursorPage<PhotoVO, String>> {

// After
        query: PhotoCursorParam,
    ) -> Result<CursorPage<PhotoResult, String>> {
```

- [ ] **Step 22: Update PhotoResult usage in get_photo_cursor_page**

In `domains/photo/src/services/photo_service.rs`, change line 113:
```rust
// Before
        let photo_vos = Self::load_photos_info(state, user_id, &photo_ids).await?;

// After
        let photo_results = Self::load_photos_info(state, user_id, &photo_ids).await?;
```

And update all subsequent references from `photo_vos` to `photo_results`.

- [ ] **Step 23: Update function signature for upload_photo**

In `domains/photo/src/services/photo_service.rs`, change line 157:
```rust
// Before
    ) -> Result<PhotoVO> {

// After
    ) -> Result<PhotoResult> {
```

- [ ] **Step 24: Update PhotoResult usage in upload_photo**

In `domains/photo/src/services/photo_service.rs`, change line 262:
```rust
// Before
        PhotoVO::from(PhotoRecord::from(photo))

// After
        PhotoResult::from(PhotoRecord::from(photo))
```

- [ ] **Step 25: Update imports in collection_service.rs**

In `domains/photo/src/services/collection_service.rs`, change line 3:
```rust
// Before
use crate::models::collection::CollectionVO;

// After
use crate::models::collection::CollectionResult;
```

- [ ] **Step 26: Update function signature for get_collection_list**

In `domains/photo/src/services/collection_service.rs`, change line 69:
```rust
// Before
    ) -> Result<Vec<CollectionVO>> {

// After
    ) -> Result<Vec<CollectionResult>> {
```

- [ ] **Step 27: Update CollectionResult usage in get_collection_list**

In `domains/photo/src/services/collection_service.rs`, change lines 80-82:
```rust
// Before
        let result: Vec<CollectionVO> = collections
            .into_iter()
            .map(|c| CollectionVO::from(c).with_generate_cover_token(&state.token_cipher))

// After
        let result: Vec<CollectionResult> = collections
            .into_iter()
            .map(|c| CollectionResult::from(c).with_generate_cover_token(&state.token_cipher))
```

- [ ] **Step 28: Update function signature for create_collection**

In `domains/photo/src/services/collection_service.rs`, change line 97:
```rust
// Before
    ) -> Result<CollectionVO> {

// After
    ) -> Result<CollectionResult> {
```

- [ ] **Step 29: Update CollectionResult usage in create_collection**

In `domains/photo/src/services/collection_service.rs`, change line 100:
```rust
// Before
        CollectionVO::from(collection).to_ok()

// After
        CollectionResult::from(collection).to_ok()
```

- [ ] **Step 30: Update function signature for create_favorite_collection**

In `domains/photo/src/services/collection_service.rs`, change line 106:
```rust
// Before
    ) -> Result<CollectionVO> {

// After
    ) -> Result<CollectionResult> {
```

- [ ] **Step 31: Update imports in collection_photo_service.rs**

In `domains/photo/src/services/collection_photo_service.rs`, change lines 8-9:
```rust
// Before
        photo::PhotoVO,

// After
        photo::PhotoResult,
```

- [ ] **Step 32: Update function signature for get_photos**

In `domains/photo/src/services/collection_photo_service.rs`, change line 36:
```rust
// Before
    ) -> Result<CursorPage<PhotoVO, String>> {

// After
    ) -> Result<CursorPage<PhotoResult, String>> {
```

- [ ] **Step 33: Update imports in comment_service.rs**

In `domains/photo/src/services/comment_service.rs`, change line 7:
```rust
// Before
        COMMENT_CURSOR_PAGE_MAX_SIZE, HOT_COMMENT_MAX_COUNT, HOT_COMMENT_MIN_LIKES, PhotoCommentVO,

// After
        COMMENT_CURSOR_PAGE_MAX_SIZE, HOT_COMMENT_MAX_COUNT, HOT_COMMENT_MIN_LIKES, PhotoCommentResult,
```

- [ ] **Step 34: Update function signature for publish**

In `domains/photo/src/services/comment_service.rs`, change line 33:
```rust
// Before
    ) -> Result<PhotoCommentVO> {

// After
    ) -> Result<PhotoCommentResult> {
```

- [ ] **Step 35: Update PhotoCommentResult usage in publish**

In `domains/photo/src/services/comment_service.rs`, change line 55:
```rust
// Before
        PhotoCommentVO::from(comment).to_ok()

// After
        PhotoCommentResult::from(comment).to_ok()
```

- [ ] **Step 36: Update function signature for get_cursor_page**

In `domains/photo/src/services/comment_service.rs`, change line 70:
```rust
// Before
    ) -> Result<CursorPage<PhotoCommentVO, DateTimeUtc>> {

// After
    ) -> Result<CursorPage<PhotoCommentResult, DateTimeUtc>> {
```

- [ ] **Step 37: Update imports in photo_controller.rs**

In `domains/photo/src/controllers/photo_controller.rs`, change line 23:
```rust
// Before
    models::photo::{DeletePhotoParam, Md5sExistParam, PhotoCursorQuery, PhotoVO},

// After
    models::photo::{DeletePhotoParam, Md5sExistParam, PhotoCursorParam, PhotoResult},
```

- [ ] **Step 38: Update return type in upload handler**

In `domains/photo/src/controllers/photo_controller.rs`, change line 54:
```rust
// Before
    ) -> Result<R<PhotoVO>> {

// After
    ) -> Result<R<PhotoResult>> {
```

- [ ] **Step 39: Update ValidatedQuery type in get_photos_cursor handler**

In `domains/photo/src/controllers/photo_controller.rs`, change line 76:
```rust
// Before
        ValidatedQuery(query): ValidatedQuery<PhotoCursorQuery>,

// After
        ValidatedQuery(query): ValidatedQuery<PhotoCursorParam>,
```

- [ ] **Step 40: Update return type in get_photos_cursor handler**

In `domains/photo/src/controllers/photo_controller.rs`, change line 77:
```rust
// Before
    ) -> Result<R<CursorPage<PhotoVO, String>>> {

// After
    ) -> Result<R<CursorPage<PhotoResult, String>>> {
```

- [ ] **Step 41: Update imports in collection_controller.rs**

In `domains/photo/src/controllers/collection_controller.rs`, change line 12:
```rust
// Before
    models::collection::{CollectionCreateParma, CollectionUpdateParam, CollectionVO},

// After
    models::collection::{CollectionCreateParam, CollectionUpdateParam, CollectionResult},
```

- [ ] **Step 42: Update ValidatedJson type in create handler**

In `domains/photo/src/controllers/collection_controller.rs`, change line 38:
```rust
// Before
        ValidatedJson(data): ValidatedJson<CollectionCreateParma>,

// After
        ValidatedJson(data): ValidatedJson<CollectionCreateParam>,
```

- [ ] **Step 43: Update return type in create handler**

In `domains/photo/src/controllers/collection_controller.rs`, change line 39:
```rust
// Before
    ) -> Result<R<CollectionVO>> {

// After
    ) -> Result<R<CollectionResult>> {
```

- [ ] **Step 44: Update return type in get_list handler**

In `domains/photo/src/controllers/collection_controller.rs`, change line 51:
```rust
// Before
    ) -> Result<R<Vec<CollectionVO>>> {

// After
    ) -> Result<R<Vec<CollectionResult>>> {
```

- [ ] **Step 45: Update imports in collection_photo_controller.rs**

In `domains/photo/src/controllers/collection_photo_controller.rs`, change lines 6-10:
```rust
// Before
        collection::{
            CollectionPhotoAddBatchParam, CollectionPhotoAddBatchResult,
            CollectionPhotoCursorPageQuery, CollectionPhotoRemoveBatchParam,
            CollectionPhotoRemoveBatchResult,
        },
        photo::PhotoVO,

// After
        collection::{
            CollectionPhotoAddBatchParam, CollectionPhotoAddBatchResult,
            CollectionPhotoCursorPageParam, CollectionPhotoRemoveBatchParam,
            CollectionPhotoRemoveBatchResult,
        },
        photo::PhotoResult,
```

- [ ] **Step 46: Update return type in get_cursor_page handler**

In `domains/photo/src/controllers/collection_photo_controller.rs`, change line 71:
```rust
// Before
    ) -> Result<R<CursorPage<PhotoVO, String>>> {

// After
    ) -> Result<R<CursorPage<PhotoResult, String>>> {
```

- [ ] **Step 47: Update ValidatedQuery type in get_cursor_page handler**

In `domains/photo/src/controllers/collection_photo_controller.rs`, change line 70:
```rust
// Before
        ValidatedQuery(query): ValidatedQuery<CollectionPhotoCursorPageQuery>,

// After
        ValidatedQuery(query): ValidatedQuery<CollectionPhotoCursorPageParam>,
```

- [ ] **Step 48: Update destructuring in get_cursor_page handler**

In `domains/photo/src/controllers/collection_photo_controller.rs`, change line 72:
```rust
// Before
        let CollectionPhotoCursorPageQuery { cursor, size } = query;

// After
        let CollectionPhotoCursorPageParam { cursor, size } = query;
```

- [ ] **Step 49: Update imports in comment_controller.rs**

In `domains/photo/src/controllers/comment_controller.rs`, change line 19:
```rust
// Before
    models::comment::{CommentCursorPageQuery, CommentPublishParam, PhotoCommentVO},

// After
    models::comment::{CommentCursorPageParam, CommentPublishParam, PhotoCommentResult},
```

- [ ] **Step 50: Update return type in publish handler**

In `domains/photo/src/controllers/comment_controller.rs`, change line 54:
```rust
// Before
    ) -> Result<R<PhotoCommentVO>> {

// After
    ) -> Result<R<PhotoCommentResult>> {
```

- [ ] **Step 51: Update ValidatedQuery type in get_cursor_page handler**

In `domains/photo/src/controllers/comment_controller.rs`, change line 70:
```rust
// Before
        ValidatedQuery(param): ValidatedQuery<CommentCursorPageQuery>,

// After
        ValidatedQuery(param): ValidatedQuery<CommentCursorPageParam>,
```

- [ ] **Step 52: Update return type in get_cursor_page handler**

In `domains/photo/src/controllers/comment_controller.rs`, change line 71:
```rust
// Before
    ) -> Result<R<CursorPage<PhotoCommentVO, DateTimeUtc>>> {

// After
    ) -> Result<R<CursorPage<PhotoCommentResult, DateTimeUtc>>> {
```

- [ ] **Step 53: Compile check photo module**

Run: `cargo check --features photo`
Expected: No compilation errors

- [ ] **Step 54: Commit photo module changes**

```bash
git add domains/photo/src/models/photo.rs domains/photo/src/models/comment.rs domains/photo/src/models/collection.rs domains/photo/src/services/photo_service.rs domains/photo/src/services/collection_service.rs domains/photo/src/services/collection_photo_service.rs domains/photo/src/services/comment_service.rs domains/photo/src/controllers/photo_controller.rs domains/photo/src/controllers/collection_controller.rs domains/photo/src/controllers/collection_photo_controller.rs domains/photo/src/controllers/comment_controller.rs
git commit -m "refactor(photo): rename model types to Param/Result convention

- PhotoVO → PhotoResult
- PhotoCursorQuery → PhotoCursorParam
- PhotoCommentVO → PhotoCommentResult
- CommentCursorPageQuery → CommentCursorPageParam
- CollectionVO → CollectionResult
- CollectionCreateParma → CollectionCreateParam (fix typo)
- CollectionPhotoCursorPageQuery → CollectionPhotoCursorPageParam"
```

---

## Task 4: Final Verification

- [ ] **Step 1: Full build verification**

Run: `cargo build --features "auth,user,photo"`
Expected: Successful build with no errors

- [ ] **Step 2: Run unit tests**

Run: `cargo test --lib`
Expected: All tests pass

- [ ] **Step 3: Run integration tests (if available)**

Run: `cargo test -p server --features "auth,user,photo" -- --test-threads=1`
Expected: All integration tests pass

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "chore: complete model naming unification

All model types now follow Param/Result convention:
- Param: input parameters (Request, Query, Param)
- Result: output objects (Response, VO, DTO)
- Row: database query results (FromQueryResult)"
```
