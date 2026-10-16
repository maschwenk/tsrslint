#!/usr/bin/env python3
"""Draw the benchmark charts from a recorded result (bench/results/<date>-<commit>.json).

The README's summary: two panels of two bars, stock tsgolint's and tsrslint's lint time and peak memory, each summed
over the projects of the result. bench/README.md's per-project charts: one row per project, a dot per binary. Shorter
is better. Same layout, fonts and palette as maschwenk/tsrs's bench/chart.py. bench/publish.py draws all of them in a
light and a dark theme; this script draws one.

    python3 bench/chart.py bench/results/<date>-<commit>.json --output /tmp/summary.svg [--theme dark]
    python3 bench/chart.py bench/results/<date>-<commit>.json --per-project wall_s --output /tmp/time.svg
"""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import xml.etree.ElementTree as ET

import run

# The summary draws them top to bottom; the multiples are stock's value over tsrslint's.
BINARIES = ("tsrslint", "stock")
NAMES = {"tsrslint": "tsrslint", "stock": "tsgolint"}
PANELS = (("wall_s", "Lint time", "faster"), ("peak_rss_bytes", "Peak memory", "less memory"))
# The README shows the light or dark chart to match the reader's GitHub theme. tsrslint gets tsrs's color, stock
# tsgolint tsrs's color for tsgo.
THEMES = {
    "light": {"tsrslint": "#087b5b", "stock": "#3178c6", "background": "#ffffff", "border": "#e5e7eb",
              "text": "#111827", "muted": "#6b7280", "faint": "#9ca3af", "track": "#f3f4f6"},
    "dark": {"tsrslint": "#3fb98c", "stock": "#58a6ff", "background": "#0d1117", "border": "#30363d",
             "text": "#e6edf3", "muted": "#9198a1", "faint": "#6e7681", "track": "#21262d"},
}
BUILDS = {"pgo-dist": "PGO dist build", "release": "cargo build --release"}
GIB = 2**30
WIDTH, HEIGHT = 1200, 420
FONT = "Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif"
# One panel: its left edge, the bars' start and full length, the first bar's center, the row pitch and bar height.
PANEL_X, BAR_X, BAR_LENGTH, ROW_Y, ROW_PITCH, BAR_HEIGHT = (48, 624), 128, 290, 228, 92, 30
DETAILS = "https://github.com/maschwenk/tsrslint/blob/main/bench/README.md"
# Marks the projects the release binaries' PGO profile is trained on, which flatters tsrslint there.
PGO_MARK = " †"


def measured(sample: dict, metric: str) -> float | None:
    value = sample.get(metric)
    return value if (sample.get("ok_runs") and isinstance(value, (int, float))
                     and math.isfinite(value) and value > 0) else None


def complete(result: dict) -> bool:
    """Every project has both binaries' wall time and peak memory, so sums and per-project rows cover the same set."""
    return bool(result.get("projects")) and all(
        measured(project.get(binary) or {}, metric) is not None
        for project in result["projects"].values() for binary in BINARIES for metric, *_ in PANELS)


def format_value(metric: str, value: float) -> str:
    return f"{value:.2f} s" if metric == "wall_s" else run.fmt_mem(value)


def total_text(metric: str, value: float) -> str:
    return f"{value:.1f} s" if metric == "wall_s" else f"{value / GIB:.1f} GiB"


def reps_text(result: dict) -> str:
    return f"median of {result['reps']} interleaved runs"


def project_label(result: dict, name: str) -> str:
    return name + (PGO_MARK if result["projects"][name].get("pgo_training") else "")


def pgo_note(result: dict) -> str:
    trained = [name for name, project in result["projects"].items() if project.get("pgo_training")]
    return f"{PGO_MARK.strip()} in the release build's PGO training set." if trained else ""


def element(parent, tag, **attrs):
    return ET.SubElement(parent, tag, {key.replace('_', '-'): str(value) for key, value in attrs.items()})


def text(parent, x, y, value, size, weight=400, fill=None, anchor=None):
    attrs = {"x": x, "y": y, "font_size": size, "font_weight": weight}
    if fill:
        attrs["fill"] = fill
    if anchor:
        attrs["text_anchor"] = anchor
    element(parent, "text", **attrs).text = value


def header(result: dict) -> tuple[str, str, str]:
    """The title, the stock version and the tsrslint build of a complete result."""
    if not complete(result):
        raise ValueError("the result does not have both binaries' wall time and peak memory for every project")
    stock = result["stock"]["version"]
    return f"tsrslint vs tsgolint {stock}", stock, BUILDS.get(result["tsrslint"].get("build"), "unknown build")


def svg(result: dict, theme: str = "light") -> str:
    """The README's summary: lint time and peak memory summed over every project, tsrslint's multiple of stock's."""
    colors = THEMES[theme]
    title, stock, build = header(result)
    cells = result["projects"]
    totals = {metric: {b: sum(measured(cell[b], metric) for cell in cells.values()) for b in BINARIES}
              for metric, *_ in PANELS}
    machine = result["machine"]
    commit = result["tsrslint"]["commit"]
    setting = (f"Summed over {len(cells)} open-source project{'s' if len(cells) != 1 else ''} · all {result['rules']} "
               f"rules · {machine.get('cpus')}-vCPU {machine.get('platform', 'Linux').split()[0]} machine")

    root = ET.Element("svg", {"xmlns": "http://www.w3.org/2000/svg", "width": str(WIDTH), "height": str(HEIGHT),
                              "viewBox": f"0 0 {WIDTH} {HEIGHT}", "role": "img", "aria-labelledby": "title description"})
    ET.SubElement(root, "title", id="title").text = title
    details = "".join(
        f"{name}: " + "; ".join(f"{NAMES[b]} {format_value('wall_s', measured(cell[b], 'wall_s'))}, "
                                f"{format_value('peak_rss_bytes', measured(cell[b], 'peak_rss_bytes'))}"
                                for b in BINARIES) + ". "
        for name, cell in cells.items())
    summary = " ".join(
        f"{label}: " + ", ".join(f"{NAMES[b]} {total_text(metric, totals[metric][b])}" for b in BINARIES) + "."
        for metric, label, _ in PANELS)
    trained = [name for name, cell in cells.items() if cell.get("pgo_training")]
    ET.SubElement(root, "desc", id="description").text = (
        f"{setting}. {summary} Shorter is better. Per project: {details}"
        + (f"{' and '.join(trained)} are in the release build's PGO training set. " if trained else "")
        + f"{reps_text(result).capitalize()}. Machine: {machine['label'].replace('`', '')}. Measured {result['date']}; "
        f"tsrslint commit {commit} ({build}); stock tsgolint {stock} (npm {result['stock']['package']}).")
    ET.SubElement(root, "metadata").text = (
        f"Generated by bench/chart.py; tsrslint commit: {commit}; measured: {result['date']}; "
        f"build: {result['tsrslint'].get('build')}. Details: {DETAILS}")

    element(root, "rect", x=0.5, y=0.5, width=WIDTH - 1, height=HEIGHT - 1, rx=20, fill=colors["background"],
            stroke=colors["border"])
    drawing = element(root, "g", font_family=FONT, fill=colors["text"])
    text(drawing, 48, 72, title, 36, 700)
    text(drawing, 48, 110, setting, 17, fill=colors["muted"])

    for (metric, label, word), left in zip(PANELS, PANEL_X):
        values = totals[metric]
        full = max(values.values())
        panel = element(drawing, "g", data_metric=metric)
        text(panel, left, 170, label, 20, 600)
        text(panel, left + BAR_X + BAR_LENGTH, 170, "shorter is better", 14, fill=colors["faint"], anchor="end")
        for row, binary in enumerate(BINARIES):
            y = ROW_Y + row * ROW_PITCH
            top = y - BAR_HEIGHT / 2
            length = max(BAR_HEIGHT, BAR_LENGTH * values[binary] / full)
            bar = element(panel, "g", data_binary=binary, data_total=f"{values[binary]:.6g}")
            element(bar, "rect", x=left + BAR_X, y=top, width=BAR_LENGTH, height=BAR_HEIGHT, rx=BAR_HEIGHT / 2,
                    fill=colors["track"])
            element(bar, "rect", x=left + BAR_X, y=top, width=f"{length:.1f}", height=BAR_HEIGHT, rx=BAR_HEIGHT / 2,
                    fill=colors[binary])
            text(bar, left + BAR_X - 14, y + 6, NAMES[binary], 18, 600 if binary == "tsrslint" else 500, anchor="end")
            value_x = left + BAR_X + BAR_LENGTH + 18
            text(bar, value_x, y + 7, total_text(metric, values[binary]), 19, 700, colors[binary])
            if binary != "stock":
                text(bar, value_x, y + 32, f"{values['stock'] / values[binary]:.1f}x {word}", 14, 500,
                     colors["muted"])

    text(drawing, 48, HEIGHT - 28, f"tsrslint {commit[:12]} ({build}) vs stock tsgolint {stock} from npm. Wall time "
         f"and peak RSS, {reps_text(result)}, measured {result['date'][:10]}.", 13, fill=colors["faint"])
    ET.indent(root, space="  ")
    return ET.tostring(root, encoding="unicode") + "\n"


# The per-project charts (bench/README.md): one row per project, a dot per binary on a shared axis. Lint time is
# linear, so the long lints read as long; peak memory is logarithmic, so the small projects do not sit on top of each
# other near zero.
DETAIL_WIDTH, DETAIL_TOP, DETAIL_PITCH, DETAIL_LEFT, DETAIL_RIGHT = 1000, 150, 34, 210, 720
DETAIL = {"wall_s": ("Lint time", "faster", "linear"), "peak_rss_bytes": ("Peak memory", "less", "log")}


def nice_step(span: float) -> float:
    """A 1-2-5 step that gives at most 8 grid lines over `span`."""
    magnitude = 10 ** math.floor(math.log10(span / 8))
    return next(m * magnitude for m in (1, 2, 5, 10) if span / (m * magnitude) <= 8)


def per_project_svg(result: dict, metric: str, theme: str = "light") -> str:
    """One project per row, sorted by stock tsgolint's lint time: stock's and tsrslint's value as dots joined by a
    line, then how many times faster (or less memory) tsrslint is."""
    colors = THEMES[theme]
    label, word, scale = DETAIL[metric]
    _, stock, build = header(result)
    cells = result["projects"]
    names = sorted(cells, key=lambda name: (-measured(cells[name]["stock"], "wall_s"), name))
    unit = 1 if metric == "wall_s" else GIB
    values = {name: {b: measured(cells[name][b], metric) / unit for b in BINARIES} for name in names}
    low = min(v for row in values.values() for v in row.values())
    high = max(v for row in values.values() for v in row.values())
    if scale == "linear":
        step = nice_step(high)
        end = math.ceil(high / step) * step
        ticks = [k * step for k in range(int(round(end / step)) + 1)]
        x_of = lambda v: DETAIL_LEFT + (DETAIL_RIGHT - DETAIL_LEFT) * v / end
    else:
        steps = [m * 10 ** e for e in range(math.floor(math.log10(low)), math.ceil(math.log10(high)) + 1)
                 for m in (1, 2, 5)]
        start = max(t for t in steps if t <= low)
        end = min((t for t in steps if t >= high), default=10 ** math.ceil(math.log10(high)))
        ticks = [t for t in steps if start <= t <= end]
        x_of = lambda v: DETAIL_LEFT + (DETAIL_RIGHT - DETAIL_LEFT) * math.log(v / start) / math.log(end / start)
    height = DETAIL_TOP + len(names) * DETAIL_PITCH + 70
    bottom = DETAIL_TOP + len(names) * DETAIL_PITCH - DETAIL_PITCH / 2
    machine = result["machine"]
    commit = result["tsrslint"]["commit"]
    title = f"{label}: tsrslint vs tsgolint {stock}"

    root = ET.Element("svg", {"xmlns": "http://www.w3.org/2000/svg", "width": str(DETAIL_WIDTH), "height": str(height),
                              "viewBox": f"0 0 {DETAIL_WIDTH} {height}", "role": "img",
                              "aria-labelledby": "title description"})
    ET.SubElement(root, "title", id="title").text = title
    ET.SubElement(root, "desc", id="description").text = " ".join(
        f"{project_label(result, name)}: "
        + ", ".join(f"{NAMES[b]} {format_value(metric, values[name][b] * unit)}" for b in BINARIES) + "."
        for name in names) + f" All {result['rules']} rules, default options. {pgo_note(result)} Smaller is better."
    ET.SubElement(root, "metadata").text = (
        f"Generated by bench/chart.py; metric: {metric}; tsrslint commit: {commit}; measured: {result['date']}.")
    element(root, "rect", x=0.5, y=0.5, width=DETAIL_WIDTH - 1, height=height - 1, rx=20, fill=colors["background"],
            stroke=colors["border"])
    drawing = element(root, "g", font_family=FONT, fill=colors["text"])
    text(drawing, 40, 58, title, 28, 700)
    text(drawing, 40, 90, f"All {result['rules']} rules on a {machine.get('cpus')}-vCPU machine, sorted by tsgolint's "
         "lint time. Smaller is better.", 15, fill=colors["muted"])
    legend_x = DETAIL_WIDTH - 40
    for binary in ("stock", "tsrslint"):
        legend_x -= 9 * len(NAMES[binary]) + 34
        element(drawing, "circle", cx=legend_x, cy=124, r=6, fill=colors[binary])
        text(drawing, legend_x + 12, 129, NAMES[binary], 14, 500, colors["muted"])

    grid = element(drawing, "g", data_scale=scale)
    for tick in ticks:
        x = x_of(tick)
        element(grid, "line", x1=f"{x:.1f}", y1=DETAIL_TOP - 18, x2=f"{x:.1f}", y2=bottom, stroke=colors["track"],
                stroke_width=1.5)
        text(grid, f"{x:.1f}", bottom + 22, f"{tick:g}", 13, fill=colors["faint"], anchor="middle")
    text(grid, (DETAIL_LEFT + DETAIL_RIGHT) / 2, bottom + 44,
         ("seconds" if metric == "wall_s" else "GiB") + (", log scale" if scale == "log" else ""), 13,
         fill=colors["muted"], anchor="middle")

    for row, name in enumerate(names):
        y = DETAIL_TOP + row * DETAIL_PITCH
        v = values[name]
        line = element(drawing, "g", data_project=name)
        text(line, DETAIL_LEFT - 18, y + 5, project_label(result, name), 14, 500, anchor="end")
        xs = [x_of(v[b]) for b in BINARIES]
        element(line, "line", x1=f"{min(xs):.1f}", y1=y, x2=f"{max(xs):.1f}", y2=y, stroke=colors["track"],
                stroke_width=5, stroke_linecap="round")
        for binary in ("stock", "tsrslint"):
            element(line, "circle", data_binary=binary, cx=f"{x_of(v[binary]):.1f}", cy=y, r=7,
                    fill=colors[binary], stroke=colors["background"], stroke_width=1.5)
        text(line, f"{max(xs) + 16:.1f}", y + 5, f"{v['stock'] / v['tsrslint']:.1f}x {word}", 13, 600,
             colors["tsrslint"])

    text(drawing, 40, height - 20, f"Wall time and peak RSS, {reps_text(result)}. tsrslint {commit[:12]} ({build}), "
         f"measured {result['date'][:10]}. {pgo_note(result)}".rstrip(), 12, fill=colors["faint"])
    ET.indent(root, space="  ")
    return ET.tostring(root, encoding="unicode") + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("result", type=Path, help="recorded benchmark result (.json)")
    parser.add_argument("--output", type=Path, required=True, help="SVG destination")
    parser.add_argument("--theme", choices=THEMES, default="light", help="color scheme (default: light)")
    parser.add_argument("--per-project", choices=DETAIL, help="draw bench/README.md's per-project chart of this "
                        "metric instead of the README's summary")
    args = parser.parse_args()
    try:
        result = json.loads(args.result.read_text())
        image = per_project_svg(result, args.per_project, args.theme) if args.per_project else svg(result, args.theme)
    except ValueError as error:
        parser.error(str(error))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(image)


if __name__ == "__main__":
    main()
