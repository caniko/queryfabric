"""Generate all QueryFabric article figures as SVG + PNG composites.
Uses only Python stdlib — no matplotlib dependency.
SVG is XML; we write it directly.
"""

from __future__ import annotations
import subprocess, sys, math
from pathlib import Path
from xml.etree import ElementTree as ET

FIGS_DIR = Path(__file__).parent / "figures"
COMPOSITES_DIR = FIGS_DIR / "composites"
COMPOSITES_DIR.mkdir(parents=True, exist_ok=True)

WIDTH = 160  # mm, linewidth
DPI = 150


def svg_begin(w_mm: int, h_mm: int) -> tuple[ET.Element, ET.Element]:
    w_pt = w_mm * 72 / 25.4
    h_pt = h_mm * 72 / 25.4
    svg = ET.Element("svg", {
        "xmlns": "http://www.w3.org/2000/svg",
        "width": f"{w_mm}mm",
        "height": f"{h_mm}mm",
        "viewBox": f"0 0 {w_mm} {h_mm}",
    })
    defs = ET.SubElement(svg, "defs")
    return svg, defs


def _escape_latex(text: str) -> str:
    """Escape LaTeX special characters in SVG text for svg package."""
    return (text
        .replace("\\", "\\textbackslash ")
        .replace("_", "\\_")
        .replace("&", "\\&")
        .replace("#", "\\#")
        .replace("%", "\\%")
        .replace("$", "\\$")
        .replace("{", "\\{")
        .replace("}", "\\}")
    )

def add_text(svg: ET.Element, x: float, y: float, text: str,
             size: float = 7, bold: bool = False, color: str = "#2C3E50",
             align: str = "middle", family: str = "sans-serif"):
    """Add a text element."""
    text = _escape_latex(text)
    weight = "bold" if bold else "normal"
    ET.SubElement(svg, "text", {
        "x": str(x), "y": str(y), "font-size": str(size),
        "font-family": family, "font-weight": weight,
        "fill": color, "text-anchor": align,
        "dominant-baseline": "central",
    }).text = text


def add_rect(svg: ET.Element, x: float, y: float, w: float, h: float,
             fill: str = "#4477AA", rx: float = 2, stroke: str | None = None,
             opacity: float = 1.0):
    attrs = {
        "x": str(x), "y": str(y), "width": str(w), "height": str(h),
        "rx": str(rx), "fill": fill, "fill-opacity": str(opacity),
    }
    if stroke:
        attrs["stroke"] = stroke
        attrs["stroke-width"] = "1"
    ET.SubElement(svg, "rect", attrs)


def add_arrow(svg: ET.Element, x1: float, y1: float, x2: float, y2: float,
              color: str = "#888", style: str = "->"):
    marker_id = f"arrowhead_{id(svg)}"
    # Check if marker already exists
    defs = svg.find("defs")
    if defs is None:
        defs = ET.SubElement(svg, "defs")
    marker = ET.SubElement(defs, "marker", {
        "id": marker_id, "markerWidth": "6", "markerHeight": "4",
        "refX": "6", "refY": "2", "orient": "auto",
    })
    ET.SubElement(marker, "polygon", {
        "points": "0 0, 6 2, 0 4", "fill": color,
    })
    ET.SubElement(svg, "line", {
        "x1": str(x1), "y1": str(y1), "x2": str(x2), "y2": str(y2),
        "stroke": color, "stroke-width": "1.2",
        "marker-end": f"url(#{marker_id})",
    })


def add_line(svg: ET.Element, x1, y1, x2, y2, color="#888", lw=0.5, dash=""):
    attrs = {"x1": str(x1), "y1": str(y1), "x2": str(x2), "y2": str(y2),
             "stroke": color, "stroke-width": str(lw)}
    if dash:
        attrs["stroke-dasharray"] = dash
    ET.SubElement(svg, "line", attrs)


def save_svg(svg: ET.Element, name: str):
    tree = ET.ElementTree(svg)
    path = COMPOSITES_DIR / f"{name}.svg"
    tree.write(path, encoding="unicode", xml_declaration=True)
    print(f"  {path.name}")
    # Also try to convert to PNG via rsvg-convert or cairosvg
    png_path = COMPOSITES_DIR / f"{name}.png"
    try:
        subprocess.run(
            ["rsvg-convert", str(path), "-o", str(png_path),
             "-d", str(DPI), "-p", str(DPI)],
            capture_output=True, check=True,
        )
        print(f"  {png_path.name}")
    except (FileNotFoundError, subprocess.CalledProcessError):
        try:
            subprocess.run(
                ["convert", str(path), "-density", str(DPI),
                 str(png_path)],
                capture_output=True, check=True,
            )
            print(f"  {png_path.name} (via ImageMagick)")
        except (FileNotFoundError, subprocess.CalledProcessError):
            print(f"  (PNG conversion skipped — no rsvg-convert or convert)")


# ── Figure 1: Architecture ───────────────────────────────────────────


def fig1_architecture():
    svg, defs = svg_begin(WIDTH, 100)

    x0, y0 = 4, 25
    bw, bh = 32, 18
    gap = 8
    colors = ["#4477AA", "#228833", "#EE7733", "#AA3377"]
    labels = ["Parse", "Bind", "Analyze", "Emit"]
    sublabels = [
        "Dialect::parse(str)",
        "bind_and_validate()",
        "analyze(bound, adapter)",
        "emit(bound, adapter)",
    ]

    for i in range(4):
        x = x0 + i * (bw + gap)
        add_rect(svg, x, y0, bw, bh, fill=colors[i], rx=3)
        add_text(svg, x + bw / 2, y0 + bh / 2 - 3,
                 labels[i], size=8, bold=True, color="white")
        add_text(svg, x + bw / 2, y0 + bh / 2 + 6,
                 sublabels[i], size=5, color="white", align="middle")

    for i in range(3):
        x1 = x0 + (i + 1) * (bw + gap) - gap
        x2 = x1 - 2
        mid_y = y0 + bh / 2
        add_arrow(svg, x2, mid_y, x1 + 2, mid_y, color="#555")

    # Catalog box
    cx, cy = x0 + 12, y0 + bh + 12
    add_rect(svg, cx, cy, 28, 10, fill="#2C3E50", rx=2)
    add_text(svg, cx + 14, cy + 3, "Catalog", size=7, bold=True, color="white")
    add_text(svg, cx + 14, cy + 7.5, "RelationSchema · FunctionRegistry",
             size=4.5, color="white", align="middle")
    add_arrow(svg, cx + 14, cy + 1, x0 + bw / 2, y0 + bh, color="#aaa")

    # Adapter box
    ax, ay = x0 + 70, y0 + bh + 12
    add_rect(svg, ax, ay, 28, 10, fill="#009988", rx=2)
    add_text(svg, ax + 14, ay + 3, "BackendAdapter", size=7, bold=True, color="white")
    add_text(svg, ax + 14, ay + 7.5, "CapabilitySet · emit()",
             size=4.5, color="white", align="middle")
    add_arrow(svg, ax + 14, ay + 1,
              x0 + 2 * (bw + gap) + bw / 2, y0 + bh, color="#aaa")

    # Provenance
    px, py = x0 + 3 * (bw + gap) + bw - 8, y0 - 16
    add_rect(svg, px, py, 24, 10, fill="#BBBBBB", rx=2)
    add_text(svg, px + 12, py + 3, "Provenance", size=7, bold=True, color="#2C3E50")
    add_text(svg, px + 12, py + 7.5, "version · catalog · dialect",
             size=4.5, color="#555", align="middle")
    add_arrow(svg, px + 12, py + 10, x0 + 3 * (bw + gap) + bw - 2, y0, color="#aaa")

    # Title
    add_text(svg, WIDTH / 2, 8, "A  Compiler Pipeline", size=10,
             bold=True, color="#2C3E50")

    save_svg(svg, "CF01-architecture")


# ── Figure 2: Portability ────────────────────────────────────────────


def fig2_portability():
    svg, defs = svg_begin(WIDTH, 70)

    add_text(svg, WIDTH / 2, 7, "A  Capability Matrix", size=9,
             bold=True, color="#2C3E50")

    # Capability matrix
    features = ["Aggregates", "CTEs", "Joins", "Windows",
                "Set Ops", "Subqueries", "Limit/Offset", "Namespaced Funcs",
                "Approx Agg", "Isolated Exec"]
    backends = ["ClickHouse", "PostgreSQL"]

    cell_w = 28
    cell_h = 5.5
    mx0, my0 = 10, 18
    top_headers_y = my0 - 3
    side_x = mx0 - 0.5

    # Headers
    for j, b in enumerate(backends):
        add_text(svg, mx0 + j * cell_w + cell_w / 2, top_headers_y,
                 b, size=6, bold=True, color="#2C3E50")

    ch_supported = [1, 1, 1, 1, 1, 1, 1, 1, 1, 1]
    pg_supported = [1, 1, 1, 1, 1, 1, 1, 0, 0, 0]

    for i, feat in enumerate(features):
        y = my0 + i * cell_h
        add_text(svg, side_x, y + cell_h / 2, feat, size=5.5,
                 color="#2C3E50", align="end")

        for j, supported in enumerate([ch_supported[i], pg_supported[i]]):
            x = mx0 + j * cell_w
            color = "#DDFFDD" if supported else "#FFDDDD"
            add_rect(svg, x, y, cell_w, cell_h, fill=color, rx=0)
            mark = "[X]" if supported else "[ ]"
            mark_color = "#1a5c1a" if supported else "#8b0000"
            add_text(svg, x + cell_w / 2, y + cell_h / 2,
                     mark, size=7, bold=True, color=mark_color)

    save_svg(svg, "CF02-portability")


# ── Figure 3: Applications ───────────────────────────────────────────


def fig3_applications():
    svg, defs = svg_begin(WIDTH, 70)

    col_w = (WIDTH - 20) / 3

    # Panel A: Python embedding
    ax0, ay0 = 5, 20
    add_rect(svg, ax0, ay0, col_w - 2, 18, fill="#4477AA", rx=3)
    add_text(svg, ax0 + (col_w - 2) / 2, ay0 + 5, "Python App", size=7,
             bold=True, color="white")
    add_text(svg, ax0 + (col_w - 2) / 2, ay0 + 13,
             "parse_syql() → emitted SQL → result", size=5,
             color="white", align="middle")

    px, py = ax0 + col_w + 2, ay0 + 6
    add_rect(svg, px, py, col_w - 2, 10, fill="#228833", rx=3)
    add_text(svg, px + (col_w - 2) / 2, py + 5,
             "QueryFabric Server (bind · analyze · emit)",
             size=5, color="white", align="middle")
    add_arrow(svg, ax0 + col_w + 1, ay0 + 9, px + 1, py + 5, color="#555")

    add_text(svg, ax0 + col_w / 2, ay0 + 22, "A  Python Embedding",
             size=7, bold=True, color="#2C3E50", align="middle")

    # Panel B: Custom adapter
    bx0, by0 = WIDTH / 3 + 5, 20
    add_rect(svg, bx0, by0, col_w - 2, 18, fill="#AA3377", rx=3)
    add_text(svg, bx0 + (col_w - 2) / 2, by0 + 5, "DuckDB Adapter",
             size=7, bold=True, color="white")
    add_text(svg, bx0 + (col_w - 2) / 2, by0 + 11,
             "impl BackendAdapter", size=5.5, color="white", align="middle")
    add_text(svg, bx0 + (col_w - 2) / 2, by0 + 16,
             "~50 lines · reuse SQL emitter", size=5,
             color="white", align="middle")

    add_text(svg, bx0 + col_w / 2, ay0 + 22, "B  Custom Backend",
             size=7, bold=True, color="#2C3E50", align="middle")

    # Panel C: Sovereignty
    cx0, cy0 = 2 * WIDTH / 3 + 5, 18
    steps = [(cx0, cy0, "#EE6677", "GDPR\nRequest"), (cx0 + 24, cy0, "#EE7733", "Access\nPolicy"),
             (cx0 + 48, cy0, "#228833", "Bundle\nBuilder"), (cx0 + 72, cy0, "#AA3377", "DOI\nMint")]
    for i, (sx, sy, sc, st) in enumerate(steps):
        add_rect(svg, sx, sy, 20, 14, fill=sc, rx=3)
        add_text(svg, sx + 10, sy + 3.5, st.split("\n")[0], size=6,
                 bold=True, color="white", align="middle")
        add_text(svg, sx + 10, sy + 10, st.split("\n")[1] if "\n" in st else "",
                 size=5, color="white", align="middle")
        if i < len(steps) - 1:
            add_arrow(svg, sx + 20, sy + 7, sx + 22, sy + 7, color="#555")

    add_text(svg, cx0 + 48, cy0 - 3, "C  Data Sovereignty",
             size=7, bold=True, color="#2C3E50", align="middle")

    save_svg(svg, "CF03-applications")


# ── Figure 4: SynDB ──────────────────────────────────────────────────


def fig4_syndb():
    svg, defs = svg_begin(WIDTH, 100)

    # Panel A: Architecture
    add_text(svg, WIDTH / 2, 8, "A  SynDB Architecture with QueryFabric",
             size=9, bold=True, color="#2C3E50")

    user_x, user_y = 5, 25
    add_rect(svg, user_x, user_y, 22, 14, fill="#4477AA", rx=3)
    add_text(svg, user_x + 11, user_y + 5, "SyQL User", size=7,
             bold=True, color="white", align="middle")
    add_text(svg, user_x + 11, user_y + 11, "Web / CLI / API",
             size=5, color="white", align="middle")

    qf_x, qf_y = 52, 22
    add_rect(svg, qf_x, qf_y, 30, 20, fill="#EE6677", rx=4, stroke="#CC3355")
    add_text(svg, qf_x + 15, qf_y + 6, "QUERYFABRIC", size=8,
             bold=True, color="white", align="middle")
    add_text(svg, qf_x + 15, qf_y + 13, "parse · bind · analyze · emit",
             size=5.5, color="white", align="middle")
    add_text(svg, qf_x + 15, qf_y + 18, "Compiler Core",
             size=5.5, color="white", align="middle")

    add_arrow(svg, user_x + 22, user_y + 7, qf_x - 1, qf_y + 10, color="#555")

    pg_x, pg_y = 95, 16
    add_rect(svg, pg_x, pg_y, 28, 14, fill="#228833", rx=3)
    add_text(svg, pg_x + 14, pg_y + 5, "PostgreSQL", size=7,
             bold=True, color="white", align="middle")
    add_text(svg, pg_x + 14, pg_y + 11, "Metadata · Auth", size=5,
             color="white", align="middle")

    ch_x, ch_y = 95, 38
    add_rect(svg, ch_x, ch_y, 28, 14, fill="#EE7733", rx=3)
    add_text(svg, ch_x + 14, ch_y + 5, "ClickHouse", size=7,
             bold=True, color="white", align="middle")
    add_text(svg, ch_x + 14, ch_y + 11, "Neuroanatomy OLAP", size=5,
             color="white", align="middle")

    add_arrow(svg, qf_x + 30, qf_y + 5, pg_x - 1, pg_y + 7, color="#555")
    add_arrow(svg, qf_x + 30, qf_y + 15, ch_x - 1, ch_y + 7, color="#555")

    # Facilities label
    add_text(svg, 12, 52, "5 shard regions", size=6, bold=True, color="#2C3E50")
    add_text(svg, 12, 58, "NRP Nautilus K8s", size=6, color="#2C3E50")

    # Panel B: Latency Benchmark
    add_line(svg, 0, 65, WIDTH, 65, color="#ccc", lw=1)
    add_text(svg, WIDTH / 2, 69, "B  Live Query Latency (Hemibrain, K8s)",
             size=9, bold=True, color="#2C3E50")

    templates = ["Nrn scan\n1K", "Nrn wide\n10K", "PK lookup", "Syn scan\n1K"]
    cold_p50 = [2154, 2941, 1869, 2043]
    warm_p50 = [2079, 3623, 1910, 2038]
    cold_p95 = [5464, 5023, 2024, 2422]
    warm_p95 = [3035, 12329, 2208, 2248]

    bar_w = 7
    chart_x0, chart_y0 = 10, 92
    max_val = max(cold_p95 + warm_p95)
    scale = 35 / max_val  # 35mm = max bar height
    bar_gap = (WIDTH - 20 - 4 * bar_w * 4) / 3

    for i, tmpl in enumerate(templates):
        base_x = chart_x0 + i * (4 * bar_w + bar_gap)
        vals = [("Cold p50", cold_p50[i], "#EE6677", 0.8),
                ("Warm p50", warm_p50[i], "#4477AA", 0.8),
                ("Cold p95", cold_p95[i], "#EE6677", 0.4),
                ("Warm p95", warm_p95[i], "#4477AA", 0.4)]

        for j, (label, val, color, opacity) in enumerate(vals):
            bar_h = val * scale
            x = base_x + j * bar_w
            add_rect(svg, x, chart_y0 - bar_h, bar_w, bar_h,
                     fill=color, rx=0, opacity=opacity)
            add_text(svg, x + bar_w / 2, chart_y0 - bar_h - 1.5,
                     f"{val}", size=4, color="#2C3E50", align="middle")

        add_text(svg, base_x + 2 * bar_w, chart_y0 + 4,
                 tmpl.replace("\n", " "), size=5, color="#2C3E50", align="middle")

    # Legend
    legend_items = [("Cold p50", "#EE6677", 0.8), ("Warm p50", "#4477AA", 0.8),
                    ("Cold p95", "#EE6677", 0.4), ("Warm p95", "#4477AA", 0.4)]
    lx, ly = 10, chart_y0 + 34
    for k, (lbl, lc, lo) in enumerate(legend_items):
        add_rect(svg, lx + k * 38, ly, 6, 4, fill=lc, rx=0, opacity=lo)
        add_text(svg, lx + k * 38 + 8, ly + 2, lbl, size=4.5, color="#2C3E50")

    save_svg(svg, "CF04-syndb")


def main():
    print("Building QueryFabric article figures...")
    fig1_architecture()
    fig2_portability()
    fig3_applications()
    fig4_syndb()
    print("Done.")


if __name__ == "__main__":
    main()
