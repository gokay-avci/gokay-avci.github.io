import assert from "node:assert/strict";
import {
  minimumImageDistance,
  periodicBondSegments,
  wrapPoint,
} from "../web/periodic-geometry.js";

const box = 10;
assert.deepEqual(wrapPoint([6, -6, 15], box), [-4, 4, -5]);

for (const [start, end] of [
  [[4.8, 0, 0], [-4.8, 0, 0]],
  [[4.8, 4.7, 0], [-4.8, -4.9, 0]],
  [[4.8, 4.7, 4.6], [-4.8, -4.9, -4.7]],
  [[1, 2, 3], [2, 3, 4]],
]) {
  const segments = periodicBondSegments(start, end, box);
  const expected = minimumImageDistance(start, end, box);
  const rendered = segments.reduce(
    (sum, segment) => sum + Math.hypot(...segment.end.map((value, axis) => value - segment.start[axis])),
    0,
  );
  assert.ok(Math.abs(rendered - expected) < 1e-9);
  assert.ok(segments.every((segment) =>
    [...segment.start, ...segment.end].every((value) => value >= -box / 2 - 1e-9 && value <= box / 2 + 1e-9)
  ));
}

console.log("periodic geometry tests passed");
