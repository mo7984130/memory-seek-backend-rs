````````````````````
Git
共三个分支
develop: 日常功能开发、合并 feature 分支 | 允许 push，禁止 force push
release: CI 验证、压测、预发布稳定       | 接受来自 develop 的 PR
main:    生产发布，永远保持可部署状态     | 只接受来自 release 的 PR
````````````````````
