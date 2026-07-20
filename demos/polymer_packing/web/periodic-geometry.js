export function wrapCentered(value, boxLength) {
  return value - boxLength * Math.floor(value / boxLength + 0.5);
}

export function wrapPoint(point, boxLength) {
  return point.map((value) => wrapCentered(value, boxLength));
}

export function minimumImageDisplacement(start, end, boxLength) {
  return end.map((value, axis) => {
    const displacement = value - start[axis];
    return displacement - boxLength * Math.round(displacement / boxLength);
  });
}

export function minimumImageDistance(start, end, boxLength) {
  return Math.hypot(...minimumImageDisplacement(start, end, boxLength));
}

export function periodicBondSegments(start, end, boxLength) {
  const halfBox = boxLength / 2;
  let current = wrapPoint(start, boxLength);
  let remaining = minimumImageDisplacement(start, end, boxLength);
  const length = Math.hypot(...remaining);
  const segments = [];
  const tolerance = 1e-10 * Math.max(1, boxLength);

  for (let crossing = 0; crossing < 4; crossing += 1) {
    let fraction = 1;
    let crossedAxes = [];
    for (let axis = 0; axis < 3; axis += 1) {
      const component = remaining[axis];
      const target = current[axis] + component;
      let candidate = Number.POSITIVE_INFINITY;
      if (component > tolerance && target > halfBox + tolerance) {
        candidate = (halfBox - current[axis]) / component;
      } else if (component < -tolerance && target < -halfBox - tolerance) {
        candidate = (-halfBox - current[axis]) / component;
      }
      if (candidate < fraction - tolerance) {
        fraction = candidate;
        crossedAxes = [axis];
      } else if (Number.isFinite(candidate) && Math.abs(candidate - fraction) <= tolerance) {
        crossedAxes.push(axis);
      }
    }

    const finish = current.map((value, axis) => value + fraction * remaining[axis]);
    if (Math.hypot(...finish.map((value, axis) => value - current[axis])) > tolerance) {
      segments.push({ start: current, end: finish, length });
    }
    if (fraction >= 1 - tolerance) break;

    current = [...finish];
    for (const axis of crossedAxes) {
      current[axis] = remaining[axis] > 0 ? -halfBox : halfBox;
    }
    remaining = remaining.map((component) => component * (1 - fraction));
  }

  return segments;
}
