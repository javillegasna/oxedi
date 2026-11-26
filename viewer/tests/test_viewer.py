"""Browser tests of the viewer page, in headless Chromium."""

from __future__ import annotations

import re

import oxedi
import pytest
from playwright.sync_api import expect
from conftest import CSP, FIXTURES, SAMPLES

SAMPLE_FIXTURE = FIXTURES / "balanced_5010_sample.txt"
TWENTY_MB = 20 * 1024 * 1024


def text_of(viewer, selector: str) -> str:
    return viewer.page.inner_text(selector)


def element_text(data: bytes, diagnostic) -> str:
    """The text of the element (or component) a diagnostic names, cut from the raw bytes."""
    text = data.decode("utf-8", errors="replace")
    element_sep, component_sep, terminator = text[3], text[104], text[105]
    segment = text.split(terminator)[diagnostic.segment].strip("\r\n")
    value = segment.split(element_sep)[diagnostic.element]
    if diagnostic.component is not None:
        value = value.split(component_sep)[diagnostic.component - 1]
    return value


def test_sample_summary_tables_and_csv(viewer, tmp_path):
    viewer.open()
    viewer.load_sample()

    summary = text_of(viewer, "#panel-summary")
    assert "ACME HEALTH PLAN" in summary
    assert "SUNRISE CLINIC" in summary

    viewer.tab("tables")
    assert viewer.page.input_value("#table-select") == "claims"
    assert "4 rows" in text_of(viewer, "#panel-tables .toolbar")

    with viewer.page.expect_download() as download:
        viewer.page.click("text=Download CSV")
    path = tmp_path / "claims.csv"
    download.value.save_as(path)
    assert download.value.suggested_filename.endswith("-claims.csv")
    lines = path.read_bytes().decode("utf-8").split("\r\n")
    assert lines[-1] == ""
    assert len(lines) - 1 == 5, "a header and 4 data lines"
    assert "claim_id" in lines[0].split(",")
    assert all(line for line in lines[1:5])


def test_csv_neutralizes_formulas_and_keeps_negative_amounts(viewer, tmp_path):
    data = SAMPLE_FIXTURE.read_bytes()
    assert b"NM1*QC*1*DOE*" in data and b"CLP*CLM1001*1*300*" in data
    data = data.replace(b"NM1*QC*1*DOE*", b"NM1*QC*1*=DOE*", 1)
    data = data.replace(b"CLP*CLM1001*1*300*", b"CLP*CLM1001*1*-300*", 1)
    viewer.open()
    viewer.load("formulas.835", data)
    viewer.wait_loaded()
    viewer.tab("tables")

    with viewer.page.expect_download() as download:
        viewer.page.click("text=Download CSV")
    path = tmp_path / "claims.csv"
    download.value.save_as(path)
    text = path.read_bytes().decode("utf-8")
    first = next(line for line in text.split("\r\n") if "CLM1001" in line)
    cells = first.split(",")
    assert "'=DOE" in cells, first
    assert not any(c == "=DOE" for c in cells), "the formula cell must not be exported as is"
    assert any(re.fullmatch(r"-300(\.0+)?", c) for c in cells), first


def test_dropped_file_with_diagnostics_links_to_text(viewer):
    path = SAMPLES / "edi835_test_file.RMT"
    expected = oxedi.parse(path.read_bytes()).diagnostics
    assert expected, "the sample must have diagnostics for this test"
    first = expected[0]

    viewer.open()
    viewer.drop(path.name, path.read_bytes())
    viewer.wait_loaded()
    assert text_of(viewer, "#diag-count") == str(len(expected))

    viewer.tab("diagnostics")
    assert text_of(viewer, "#panel-diagnostics .diag-list li:first-child .text") == str(first)
    viewer.page.click("#panel-diagnostics .diag-list li:first-child .jump")

    assert viewer.page.is_visible("#panel-text")
    current = viewer.page.locator("#panel-text .line.current")
    assert current.count() == 1
    assert current.locator(".no").inner_text() == str(first.segment)
    if first.element is not None:
        mark = current.locator("mark")
        assert mark.count() == 1
        assert mark.inner_text() == element_text(path.read_bytes(), first)


def test_non_835_shows_the_core_error_then_the_sample_loads(viewer):
    with pytest.raises(Exception) as raised:
        oxedi.parse(b"%PDF-1.7")
    viewer.open()
    viewer.drop("report.pdf", b"%PDF-1.7")
    viewer.page.wait_for_selector("#alert:not([hidden])")
    alert = text_of(viewer, "#alert")
    assert alert == f"report.pdf: {raised.value}"
    assert "\n" not in alert.strip()
    assert viewer.page.is_hidden("#results")

    viewer.load_sample()
    assert viewer.page.is_hidden("#alert")
    assert "ACME HEALTH PLAN" in text_of(viewer, "#panel-summary")


@pytest.mark.parametrize(
    ("replacement", "shown"),
    [
        # One invalid byte decodes to one replacement character.
        (b"SUN\xd1ISE", "SUN\ufffdISE CLINIC"),
        # Two bytes are one character: a page that sliced decoded text by byte offsets would drift.
        (b"SUN\xc3\x91ISE", "SUN\u00d1ISE CLINIC"),
    ],
    ids=["latin1", "utf8-two-bytes"],
)
def test_non_utf8_bytes_keep_offsets_aligned(viewer, tmp_path, replacement, shown):
    data = SAMPLE_FIXTURE.read_bytes()
    # Only the payee name changes: the ISA segment has fixed widths.
    assert b"N1*PE*SUNRISE CLINIC" in data
    changed = tmp_path / "changed.835"
    changed.write_bytes(data.replace(b"N1*PE*SUNRISE", b"N1*PE*" + replacement, 1))
    viewer.open()
    viewer.load(changed)
    viewer.wait_loaded()

    assert shown in text_of(viewer, "#panel-summary")

    viewer.tab("tables")
    viewer.page.click("#panel-tables button[data-segment]")
    current = viewer.page.locator("#panel-text .line.current .raw")
    # Line breaks before a segment are shown as glyphs; the segment itself starts at its own id.
    text = current.inner_text().lstrip("\u240d\u240a")
    assert text.startswith("CLP*CLM1001"), "the segment after the changed bytes is not shifted"


def test_csp_blocks_network_and_is_declared(viewer):
    viewer.expect_violations = True
    viewer.open()
    content = viewer.page.evaluate(
        "document.querySelector('meta[http-equiv=\"Content-Security-Policy\"]').content"
    )
    assert content == CSP
    rejected = viewer.page.evaluate(
        "fetch('/').then(() => false, () => true)"
    )
    assert rejected is True
    assert any("connect-src" in v for v in viewer.violations())


def test_largest_sample_is_shown_in_under_a_second(viewer):
    path = SAMPLES / "edi835_test_united.rmt"
    viewer.open()
    # Time from the input's change event to the results becoming visible. The page's error
    # alert or a 10 s bound ends the wait, so a regression fails with a message, not a hang.
    viewer.page.evaluate(
        """() => {
          window.__timing = new Promise((resolve) => {
            let t0 = 0;
            const results = document.getElementById('results');
            const alert = document.getElementById('alert');
            const done = (outcome) => { observer.disconnect(); clearTimeout(bound); resolve(outcome); };
            window.addEventListener('change', () => { t0 = performance.now(); }, true);
            const observer = new MutationObserver(() => {
              if (!results.hidden) done({ ms: performance.now() - t0 });
              else if (!alert.hidden) done({ alert: alert.textContent });
            });
            observer.observe(results, { attributes: true, attributeFilter: ['hidden'] });
            observer.observe(alert, { attributes: true, attributeFilter: ['hidden'] });
            const bound = setTimeout(() => done({ timeout: performance.now() - t0 }), 10000);
          });
        }"""
    )
    viewer.load(path)
    outcome = viewer.page.evaluate("window.__timing")
    name = f"{path.name} ({path.stat().st_size} bytes)"
    assert "alert" not in outcome, f"{name} raised the alert: {outcome.get('alert')}"
    assert "timeout" not in outcome, f"{name} was not shown after {outcome.get('timeout'):.0f} ms (limit 1000 ms)"
    elapsed = outcome["ms"]
    print(f"\nviewer timing: {name} shown in {elapsed:.0f} ms")
    assert elapsed < 1000, f"{name} took {elapsed:.0f} ms (limit 1000 ms)"


def test_size_guard_function(viewer):
    viewer.open()
    result = viewer.page.evaluate(
        """async (limit) => {
          const { needsConfirmation } = await import('/js/load.js');
          const big = new File([new Uint8Array(limit + 1)], 'big.835');
          const exact = new File([new Uint8Array(limit)], 'exact.835');
          const small = new File([new Uint8Array(100)], 'small.835');
          return [needsConfirmation(big), needsConfirmation(exact), needsConfirmation(small)];
        }""",
        TWENTY_MB,
    )
    assert result == [True, False, False]


def test_large_file_asks_before_parsing(viewer):
    viewer.open()
    viewer.load_sample()
    before = text_of(viewer, "#panel-summary")
    dialogs: list[tuple[str, str]] = []

    def dismiss(dialog) -> None:
        dialogs.append((dialog.type, dialog.message))
        dialog.dismiss()

    viewer.page.once("dialog", dismiss)
    viewer.load("big.835", bytes(TWENTY_MB + 1))
    viewer.page.wait_for_timeout(300)
    assert len(dialogs) == 1
    assert dialogs[0][0] == "confirm"
    assert "big.835" in dialogs[0][1] and "20.0 MB" in dialogs[0][1]
    assert viewer.page.is_visible("#results")
    assert text_of(viewer, "#panel-summary") == before
    assert viewer.page.is_hidden("#alert")
    assert viewer.page.is_hidden("#status")

    # Accepting parses it, and the (non-835) bytes give the core's error.
    viewer.page.once("dialog", lambda dialog: dialog.accept())
    viewer.load("big.835", bytes(TWENTY_MB + 1))
    viewer.page.wait_for_selector("#alert:not([hidden])")
    assert text_of(viewer, "#alert").startswith("big.835: ")


def test_unterminated_last_segment_is_visible_in_text(viewer):
    data = SAMPLE_FIXTURE.read_bytes().rstrip(b"\r\n")
    assert data.endswith(b"IEA*1*000000101~")
    data = data + b"ZZ*TAIL"
    viewer.open()
    viewer.load("unterminated.835", data)
    viewer.wait_loaded()
    viewer.tab("text")
    # The last row of the list holds the unterminated bytes.
    viewer.page.evaluate(
        "document.querySelector('#panel-text .textview').scrollTop = 1e9"
    )
    viewer.page.wait_for_selector("#panel-text .line .raw:text-is('ZZ*TAIL')")


def test_hide_patient_data_masks_cells_and_csv_and_restores(viewer, tmp_path):
    data = SAMPLE_FIXTURE.read_bytes()
    name = b"DOE"
    assert b"NM1*QC*1*DOE*" in data
    viewer.open()
    viewer.load("masked.835", data)
    viewer.wait_loaded()
    viewer.tab("tables")
    assert name.decode() in text_of(viewer, "#panel-tables")

    viewer.page.check("#mask")
    viewer.page.wait_for_selector("#panel-tables:not([hidden])", state="attached")
    # The re-parse is asynchronous: wait for a token before reading the grid. (The page's
    # CSP forbids eval, so this uses an expectation, not wait_for_function.)
    panel = viewer.page.locator("#panel-tables")
    expect(panel).not_to_contain_text("DOE")
    expect(panel).to_contain_text(re.compile(r"[0-9a-f]{16}"))
    viewer.tab("tables")
    grid = text_of(viewer, "#panel-tables")
    assert "DOE" not in grid
    assert re.search(r"\b[0-9a-f]{16}\b", grid)

    with viewer.page.expect_download() as download:
        viewer.page.click("text=Download CSV")
    path = tmp_path / "claims.csv"
    download.value.save_as(path)
    csv = path.read_bytes().decode("utf-8")
    assert "DOE" not in csv
    assert re.search(r"(^|,)[0-9a-f]{16}(,|\r)", csv)

    viewer.page.uncheck("#mask")
    expect(viewer.page.locator("#panel-tables")).to_contain_text("DOE")
    viewer.tab("tables")
    assert "DOE" in text_of(viewer, "#panel-tables")


def test_the_toggle_warns_that_the_text_view_is_not_masked(viewer):
    viewer.open()
    assert viewer.page.is_hidden("#mask-note")
    viewer.page.check("#mask")
    assert "original file" in text_of(viewer, "#mask-note")
    assert "table filter stays as typed" in text_of(viewer, "#mask-note")
    viewer.page.uncheck("#mask")
    assert viewer.page.is_hidden("#mask-note")


def test_findings_on_flagged_places_are_masked_in_the_diagnostics_list(viewer):
    path = FIXTURES / "masked_findings_5010_sample.txt"
    plain = [d.datum for d in oxedi.parse(path.read_bytes()).diagnostics]
    assert len(plain) == 5
    viewer.open()
    viewer.load(path.name, path.read_bytes())
    viewer.wait_loaded()
    viewer.tab("diagnostics")
    assert "20241341" in text_of(viewer, "#panel-diagnostics .diag-list")

    viewer.page.check("#mask")
    panel = viewer.page.locator("#panel-diagnostics .diag-list")
    expect(panel).not_to_contain_text("20241341")
    viewer.tab("diagnostics")
    shown = text_of(viewer, "#panel-diagnostics .diag-list")
    for datum in plain:
        assert (datum.decode() if isinstance(datum, bytes) else datum) not in shown
    assert len(re.findall(r'datum "[0-9a-f]{16}"', shown)) == 5


SLOW_READS = """() => {
  const original = Blob.prototype.arrayBuffer;
  Blob.prototype.arrayBuffer = async function () {
    const buffer = await original.call(this);
    await new Promise((resolve) => setTimeout(resolve, 700));
    return buffer;
  };
}"""


def test_the_toggle_keeps_the_tab_and_the_filter_of_the_open_file(viewer):
    data = SAMPLE_FIXTURE.read_bytes()
    viewer.open()
    viewer.load("keep.835", data)
    viewer.wait_loaded()
    viewer.tab("tables")
    viewer.page.fill("#table-filter", "DOE")
    panel = viewer.page.locator("#panel-tables")
    expect(panel).to_contain_text(re.compile(r"\d+ of \d+ rows"))

    viewer.page.check("#mask")
    # Rows matching "DOE" are filtered on the masked values, so none show it any more.
    expect(panel).not_to_contain_text("DOE")
    assert viewer.page.get_attribute("#tab-tables", "aria-selected") == "true"
    assert viewer.page.input_value("#table-filter") == "DOE"
    expect(viewer.page.locator("#panel-tables .toolbar span[role=status]")).to_have_text(re.compile(r"^0 of \d+ rows$"))

    viewer.page.fill("#table-filter", "")
    expect(panel).to_contain_text(re.compile(r"[0-9a-f]{16}"))


def test_a_toggle_during_a_load_ends_on_the_new_file_masked(viewer):
    first = SAMPLE_FIXTURE.read_bytes()
    second = (FIXTURES / "masked_findings_5010_sample.txt").read_bytes()
    viewer.open()
    viewer.load("a.835", first)
    viewer.wait_loaded()
    viewer.page.evaluate(SLOW_READS)
    viewer.load("b.835", second)
    viewer.page.check("#mask")
    viewer.tab("tables")
    panel = viewer.page.locator("#panel-tables")
    expect(panel).to_contain_text(re.compile(r"[0-9a-f]{16}"))
    expect(viewer.page.locator("#panel-summary")).to_contain_text("b.835")
    assert "DOE" not in text_of(viewer, "#panel-tables")


def test_two_quick_toggles_during_a_load_end_on_the_new_file_unmasked(viewer):
    first = SAMPLE_FIXTURE.read_bytes()
    second = (FIXTURES / "masked_findings_5010_sample.txt").read_bytes()
    viewer.open()
    viewer.load("a.835", first)
    viewer.wait_loaded()
    viewer.page.evaluate(SLOW_READS)
    viewer.load("b.835", second)
    viewer.page.check("#mask")
    viewer.page.uncheck("#mask")
    expect(viewer.page.locator("#panel-summary")).to_contain_text("b.835")
    viewer.tab("tables")
    expect(viewer.page.locator("#panel-tables")).to_contain_text("DOE")
    assert not re.search(r"\b[0-9a-f]{16}\b", text_of(viewer, "#panel-tables"))
