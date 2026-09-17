import { Presentation, PresentationFile } from "@oai/artifact-tool";

const output = new URL("./fixtures/realistic-presentation.pptx", import.meta.url);
const deck = Presentation.create({ slideSize: { width: 1280, height: 720 } });

function text(slide, name, value, position, placeholderType) {
  const shape = slide.shapes.add({
    geometry: "textbox", name, position, fill: "none",
    line: { style: "solid", fill: "none", width: 0 }, placeholderType,
  });
  shape.text = value;
  shape.text.style = { fontSize: 24, color: "slate-900", typeface: "Aptos" };
  return shape;
}

const first = deck.slides.add();
first.background.fill = "white";
const title = text(first, "Title placeholder", "OpenSuite PPTX interoperability", { left: 72, top: 56, width: 1040, height: 64 }, "title");
title.text.style = { fontSize: 34, bold: true, color: "slate-950", typeface: "Aptos Display" };
const body = text(first, "Body placeholder", [
  [{ run: "Baseline ", textStyle: { bold: true } }, { run: "& source remains", textStyle: { italic: true, color: "#2563eb" } }],
  [{ run: "Second paragraph remains unchanged." }],
], { left: 84, top: 160, width: 600, height: 160 }, "body");
body.text.style = { fontSize: 22, color: "slate-800", typeface: "Aptos" };
first.charts.add("bar", { position: { left: 730, top: 160, width: 420, height: 260 }, title: "Fixture values", categories: ["One", "Two"], series: [{ name: "Values", values: [2, 4], fill: "accent1" }], hasLegend: false });

const second = deck.slides.add();
second.background.fill = "slate-50";
text(second, "Slide two title", "Second slide", { left: 72, top: 56, width: 900, height: 64 }, "title").text.style = { fontSize: 34, bold: true, color: "slate-950", typeface: "Aptos Display" };
text(second, "Slide two text", "Unrelated text stays on slide two.", { left: 84, top: 170, width: 760, height: 80 });

const third = deck.slides.add();
third.background.fill = "white";
text(third, "Slide three title", "Third slide", { left: 72, top: 56, width: 900, height: 64 }, "title").text.style = { fontSize: 34, bold: true, color: "slate-950", typeface: "Aptos Display" };
text(third, "Slide three text", "Inspection order follows presentation order.", { left: 84, top: 170, width: 760, height: 80 });

await (await PresentationFile.exportPptx(deck)).save(output.pathname);
