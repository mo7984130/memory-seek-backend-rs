#!/usr/bin/env python3
"""压测结果比对与判定。

输入是 e2e crate 产出的 JSON 报告(--report-json), 输出 markdown 报告 + 退出码:
- baseline 模式: 把本轮报告写为基线(--write-baseline), 不判定;
- compare 模式: 与基线逐场景比对 P95, 叠加绝对阈值判定。

基线缺失时只做绝对阈值判定(首次接入/缓存未命中不会误报失败)。

阈值(环境变量可覆盖):
  MIN_PASS_RATE            有效通过率下限, 默认 0.99
  MAX_P95_REGRESSION_PCT   单场景 P95 相对基线允许的最大涨幅(%), 默认 20
  MIN_RPS_RATIO            整体吞吐相对基线的下限倍率, 默认 0.9
  MAX_TIMEOUT_RATE         超时率上限, 默认 0.005
  MAX_TEARDOWN_FAIL_RATE   收尾失败率上限(收尾失败不影响通过率, 这里单独看), 默认 0.02
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
from typing import Any


def env_float(name: str, default: float) -> float:
    raw = os.environ.get(name)
    return float(raw) if raw else default


MIN_PASS_RATE = env_float("MIN_PASS_RATE", 0.99)
MAX_P95_REGRESSION_PCT = env_float("MAX_P95_REGRESSION_PCT", 20.0)
MIN_RPS_RATIO = env_float("MIN_RPS_RATIO", 0.9)
MAX_TIMEOUT_RATE = env_float("MAX_TIMEOUT_RATE", 0.005)
MAX_TEARDOWN_FAIL_RATE = env_float("MAX_TEARDOWN_FAIL_RATE", 0.02)


def load(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as handle:
        return json.load(handle)


def fmt_ms(value: float) -> str:
    return f"{value:.1f}ms"


def fmt_duration(seconds: float) -> str:
    return f"{seconds:.2f}s" if seconds >= 1 else f"{seconds * 1000:.0f}ms"


def scenario_map(report: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {item["name"]: item for item in report.get("scenarios", [])}


def evaluate(
    current: dict[str, Any], baseline: dict[str, Any] | None
) -> tuple[list[str], str]:
    """返回 (未通过原因列表, markdown 报告)。"""
    overall = current["overall"]
    environment = current["environment"]
    scenarios = current.get("scenarios", [])
    baseline_scenarios = scenario_map(baseline) if baseline else {}
    baseline_overall = baseline["overall"] if baseline else None

    times = max(overall["times"], 1)
    # 通过 = 轮次 - run 失败 - 超时 - 校验未通过(三者互斥, 与框架口径一致)
    passed_rounds = max(
        times - overall["failures"] - overall["timeouts"] - overall["validate_failures"],
        0,
    )
    pass_rate = passed_rounds / times
    timeout_rate = overall["timeouts"] / times
    teardown_total = sum(item.get("teardown_count", 0) for item in scenarios)
    teardown_failures = overall.get("teardown_failures", 0)
    teardown_fail_rate = teardown_failures / max(teardown_total, 1)

    reasons: list[str] = []
    if pass_rate < MIN_PASS_RATE:
        reasons.append(
            f"有效通过率 {pass_rate:.4f} 低于下限 {MIN_PASS_RATE:.4f}({passed_rounds}/{times})"
        )
    if timeout_rate > MAX_TIMEOUT_RATE:
        reasons.append(
            f"超时率 {timeout_rate:.4f} 超过上限 {MAX_TIMEOUT_RATE:.4f}"
            f"({overall['timeouts']}/{times})"
        )
    if teardown_fail_rate > MAX_TEARDOWN_FAIL_RATE:
        reasons.append(
            f"收尾失败率 {teardown_fail_rate:.4f} 超过上限 {MAX_TEARDOWN_FAIL_RATE:.4f}"
            f"({teardown_failures}/{teardown_total})——存在资源泄漏风险"
        )
    if overall.get("interrupted"):
        reasons.append("存在被中断的场景(优雅关闭)")

    if baseline_overall and baseline_overall["rps"] > 0:
        ratio = overall["rps"] / baseline_overall["rps"]
        if ratio < MIN_RPS_RATIO:
            reasons.append(
                f"整体吞吐 {overall['rps']:.1f} req/s 低于基线的 {MIN_RPS_RATIO:.2f} 倍"
                f"(基线 {baseline_overall['rps']:.1f} req/s, 当前 {ratio:.2f}x)"
            )

    rows = [
        "| 场景 | 轮次 | 通过率 | RPS | P50 | P95 | P99 | 失败 | 超时 | 收尾失败 | P95 vs 基线 |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for item in sorted(scenarios, key=lambda s: s["name"]):
        item_times = max(item["times"], 1)
        item_pass = max(
            item_times
            - item["failures"]
            - item["timeouts"]
            - item["validate_failures"],
            0,
        ) / item_times
        base = baseline_scenarios.get(item["name"])
        if not base or base["p95_ms"] <= 0:
            delta = "-"
        else:
            change = (item["p95_ms"] - base["p95_ms"]) / base["p95_ms"] * 100
            delta = f"{change:+.1f}%"
            if change > MAX_P95_REGRESSION_PCT:
                reasons.append(
                    f"{item['name']} 的 P95 {fmt_ms(item['p95_ms'])} 较基线"
                    f"{fmt_ms(base['p95_ms'])} 恶化 {change:.1f}%(上限 {MAX_P95_REGRESSION_PCT:.0f}%)"
                )
        rows.append(
            "| {name} | {times} | {pass_rate:.1%} | {rps:.1f} | {p50} | {p95} | {p99} "
            "| {failures} | {timeouts} | {teardown} | {delta} |".format(
                name=item["name"],
                times=item["times"],
                pass_rate=item_pass,
                rps=item["rps"],
                p50=fmt_ms(item["p50_ms"]),
                p95=fmt_ms(item["p95_ms"]),
                p99=fmt_ms(item["p99_ms"]),
                failures=item["failures"] + item["validate_failures"],
                timeouts=item["timeouts"],
                teardown=item["teardown_failures"],
                delta=delta,
            )
        )

    mode = environment["mode"]
    scope = (
        f"运行 {environment['planned_times']} 轮"
        if mode == "times"
        else fmt_duration(float(environment["planned_duration_secs"]))
    )
    if reasons:
        conclusion = "❌ 未通过"
    elif baseline:
        conclusion = "✅ 通过"
    else:
        conclusion = "✅ 通过(无基线, 仅绝对阈值)"

    throughput = f"{overall['rps']:.1f} req/s"
    if baseline_overall:
        throughput += f" (基线 {baseline_overall['rps']:.1f} req/s)"

    # 基线口径不一致时给出提醒: 跨口径比较只能看趋势, 不能当回归结论
    baseline_note = []
    if baseline:
        base_env = baseline.get("environment", {})
        for key, label in (
            ("mode", "模式"),
            ("concurrency", "并发"),
            ("planned_duration_secs", "时长"),
            ("scenario_exclude", "场景排除"),
            ("server_url", "目标"),
        ):
            if base_env.get(key) != environment.get(key):
                baseline_note.append(
                    f"{label}: 基线 {base_env.get(key)} vs 当前 {environment.get(key)}"
                )

    lines = [
        "## 压测报告",
        "",
        f"- 结论: {conclusion}",
        f"- 被测目标: {environment['server_url']}",
        f"- 参数: {mode} / {scope} / 并发 {environment['concurrency']} / 种子准备 {environment['prepared']}",
        f"- 总量: {overall['times']} 轮, 有效通过率 {pass_rate:.2%}, 超时率 {timeout_rate:.4%}, "
        f"收尾失败 {teardown_failures}/{teardown_total}",
        f"- 吞吐: {throughput}",
        "",
    ]
    if baseline_note:
        lines.append(f"⚠️ 基线口径与本次不同({'; '.join(baseline_note)}), 对比仅供趋势参考。")
        lines.append("")
    if reasons:
        lines.append("未通过原因:")
        lines.extend(f"- {reason}" for reason in reasons)
        lines.append("")
    lines.append(
        "> 延迟分位只统计被测接口(run 阶段);setup/validate/teardown 不计入分位, 但计入整体耗时与吞吐。"
    )
    lines.append("")
    lines.extend(rows)
    lines.append("")
    return reasons, "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="压测报告比对")
    parser.add_argument("--current", required=True, help="本轮 JSON 报告")
    parser.add_argument("--baseline", help="基线 JSON 报告(compare 模式)")
    parser.add_argument("--markdown", help="markdown 报告输出路径")
    parser.add_argument(
        "--write-baseline", help="把本轮报告写为该路径的基线(baseline 模式), 不做判定"
    )
    args = parser.parse_args()

    if args.write_baseline:
        current = load(args.current)
        os.makedirs(os.path.dirname(args.write_baseline) or ".", exist_ok=True)
        shutil.copyfile(args.current, args.write_baseline)
        summary = (
            f"已刷新基线: {args.write_baseline}\n"
            f"参数: {current['environment']}\n"
            f"轮次: {current['overall']['times']}, 吞吐: {current['overall']['rps']:.1f} req/s"
        )
        print(summary)
        if args.markdown:
            with open(args.markdown, "w", encoding="utf-8") as handle:
                handle.write(f"## 压测基线已刷新\n\n```\n{summary}\n```\n")
        return 0

    current = load(args.current)
    baseline = None
    if args.baseline and os.path.exists(args.baseline):
        baseline = load(args.baseline)
    elif args.baseline:
        print(f"基线不存在({args.baseline}), 跳过对比, 仅做绝对阈值判定")

    reasons, markdown = evaluate(current, baseline)
    print(markdown)
    if args.markdown:
        with open(args.markdown, "w", encoding="utf-8") as handle:
            handle.write(markdown)

    if reasons:
        print(f"判定: FAIL({len(reasons)} 项)", file=sys.stderr)
        return 1
    print("判定: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
