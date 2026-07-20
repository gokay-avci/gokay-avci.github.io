import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { periodicBondSegments } from "./periodic-geometry.js?v=20260720d";

const stretchColor = new THREE.Color(0xff3b4e);

function architectureColor(color, architecture, beadIndex, chainLength, degree, role, chain) {
  const contour = beadIndex / Math.max(1, chainLength - 1);
  const chainLightness = 0.63 + 0.05 * Math.sin((chain + 1) * 2.399);
  if (architecture === 0) return color.setHSL(0.60 - 0.16 * contour, 0.92, chainLightness);
  if (architecture === 1) return color.setHSL((0.91 + contour) % 1, 0.88, chainLightness);
  if (architecture === 2) {
    if (role === 4) return color.setHex(0xffe08a);
    if (role === 2) return color.setHex(0xff7a6e);
    if (role === 1) return color.setHex(0x72f2a4);
    return color.setHex(0x70c7ff);
  }
  if (architecture === 3) {
    if (role === 2) return color.setHex(0xffa347);
    if (role === 3) return color.setHex(0xd988ff);
    if (role === 1) return color.setHex(0xff77b7);
    return color.setHex(0x58d5ff);
  }
  return color.setHex(0x63b3ff);
}

export class StructureViewer {
  constructor(canvas, { onSelect } = {}) {
    this.canvas = canvas;
    this.onSelect = onSelect;
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x0e1528);
    this.scene.fog = new THREE.FogExp2(0x0e1528, 0.025);
    this.camera = new THREE.PerspectiveCamera(42, 1, 0.01, 500);
    this.camera.position.set(7, 5, 8);
    this.renderer = new THREE.WebGLRenderer({
      canvas,
      antialias: false,
      alpha: false,
      powerPreference: "low-power",
    });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.25));
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enableDamping = false;
    this.controls.minDistance = 1;
    this.controls.maxDistance = 80;
    this.raycaster = new THREE.Raycaster();
    this.pointer = new THREE.Vector2();
    this.atomMesh = null;
    this.bondMesh = null;
    this.voidPoints = null;
    this.cell = null;
    this.selected = -1;
    this.lastExtent = 5;
    this.lastPoints = [];
    this.renderQueued = false;

    this.scene.add(new THREE.HemisphereLight(0xc8ddff, 0x1b2135, 2.1));
    const key = new THREE.DirectionalLight(0xffffff, 3.2);
    key.position.set(5, 8, 6);
    this.scene.add(key);
    const rim = new THREE.DirectionalLight(0x7ba7ff, 2.0);
    rim.position.set(-6, -2, -4);
    this.scene.add(rim);

    this.selectionHalo = new THREE.Mesh(
      new THREE.SphereGeometry(0.145, 12, 8),
      new THREE.MeshBasicMaterial({ color: 0xffffff, wireframe: true, transparent: true, opacity: 0.8 }),
    );
    this.selectionHalo.visible = false;
    this.scene.add(this.selectionHalo);

    canvas.addEventListener("click", (event) => this.pick(event));
    canvas.addEventListener("pointermove", (event) => this.hover(event));
    this.controls.addEventListener("change", () => this.requestRender());
    window.addEventListener("resize", () => this.resize());
    this.resize();
  }

  clearObject(object) {
    if (!object) return;
    this.scene.remove(object);
    object.geometry?.dispose();
    if (Array.isArray(object.material)) object.material.forEach((material) => material.dispose());
    else object.material?.dispose();
  }

  update({ points, chains, beadIndices, chainLengths, beadTypes, degrees, architecture, bonds, targetGene, extent, showBonds, showCell, showVoid }) {
    const shouldFrame = this.lastPoints.length !== points.length
      || extent > this.lastExtent * 1.35
      || extent < this.lastExtent * 0.74;
    this.lastPoints = points;
    this.lastExtent = extent;
    this.clearObject(this.atomMesh);
    this.clearObject(this.bondMesh);
    this.clearObject(this.voidPoints);
    this.clearObject(this.cell);

    const detail = points.length > 1000 ? [8, 6] : [12, 8];
    const atomGeometry = new THREE.SphereGeometry(Math.max(0.08, extent * 0.018), ...detail);
    const atomMaterial = new THREE.MeshPhysicalMaterial({
      color: 0xffffff,
      vertexColors: true,
      roughness: 0.30,
      metalness: 0.06,
      clearcoat: 0.72,
      clearcoatRoughness: 0.20,
      emissive: 0x10192d,
      emissiveIntensity: 0.32,
      fog: false,
    });
    this.atomMesh = new THREE.InstancedMesh(atomGeometry, atomMaterial, points.length);
    this.atomMesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    const matrix = new THREE.Matrix4();
    const rotation = new THREE.Quaternion();
    const atomScale = new THREE.Vector3();
    const atomPosition = new THREE.Vector3();
    const color = new THREE.Color();
    points.forEach((point, atom) => {
      const role = Math.max(0, beadTypes[atom] ?? 0);
      const scaleFactor = role === 4 ? 1.38 : role === 2 ? 1.22 : role === 1 ? 1.08 : 1.0;
      atomScale.setScalar(scaleFactor);
      atomPosition.fromArray(point);
      matrix.compose(atomPosition, rotation, atomScale);
      this.atomMesh.setMatrixAt(atom, matrix);
      architectureColor(
        color,
        architecture,
        beadIndices[atom] ?? atom,
        chainLengths[atom] ?? points.length,
        degrees[atom] ?? 0,
        role,
        chains[atom] ?? 0,
      );
      if (chains[atom] === targetGene) color.lerp(new THREE.Color(0xf2cd67), 0.48);
      this.atomMesh.setColorAt(atom, color);
    });
    this.atomMesh.instanceMatrix.needsUpdate = true;
    if (this.atomMesh.instanceColor) this.atomMesh.instanceColor.needsUpdate = true;
    this.atomMesh.computeBoundingSphere();
    this.scene.add(this.atomMesh);

    if (showBonds) {
      const pairs = bonds ?? [];
      const periodicSegments = pairs.flatMap(([first, second]) =>
        periodicBondSegments(points[first], points[second], extent)
      );
      if (periodicSegments.length > 5000) {
        const segmentPositions = new Float32Array(periodicSegments.length * 6);
        const segmentColors = new Float32Array(periodicSegments.length * 6);
        periodicSegments.forEach((segment, index) => {
          segmentPositions.set([...segment.start, ...segment.end], index * 6);
          const bondColor = segment.length > 1.5 ? stretchColor : color.setHex(0x91b5e8);
          segmentColors.set([...bondColor.toArray(), ...bondColor.toArray()], index * 6);
        });
        const geometry = new THREE.BufferGeometry();
        geometry.setAttribute("position", new THREE.BufferAttribute(segmentPositions, 3));
        geometry.setAttribute("color", new THREE.BufferAttribute(segmentColors, 3));
        this.bondMesh = new THREE.LineSegments(
          geometry,
          new THREE.LineBasicMaterial({
            vertexColors: true,
            transparent: true,
            opacity: 0.76,
            fog: false,
            toneMapped: false,
          }),
        );
        this.scene.add(this.bondMesh);
      } else {
        const geometry = new THREE.CylinderGeometry(extent * 0.006, extent * 0.006, 1, 6);
        const material = new THREE.MeshBasicMaterial({
          color: 0xffffff,
          vertexColors: true,
          fog: false,
          toneMapped: false,
        });
        this.bondMesh = new THREE.InstancedMesh(geometry, material, periodicSegments.length);
        const up = new THREE.Vector3(0, 1, 0);
        const start = new THREE.Vector3();
        const end = new THREE.Vector3();
        const midpoint = new THREE.Vector3();
        const direction = new THREE.Vector3();
        const quaternion = new THREE.Quaternion();
        const scale = new THREE.Vector3();
        periodicSegments.forEach((segment, index) => {
          start.fromArray(segment.start);
          end.fromArray(segment.end);
          midpoint.copy(start).add(end).multiplyScalar(0.5);
          direction.copy(end).sub(start);
          const segmentLength = direction.length();
          quaternion.setFromUnitVectors(up, direction.normalize());
          scale.set(1, segmentLength, 1);
          matrix.compose(midpoint, quaternion, scale);
          this.bondMesh.setMatrixAt(index, matrix);
          this.bondMesh.setColorAt(
            index,
            segment.length > 1.5 ? stretchColor : color.setHex(0x91b5e8),
          );
        });
        this.bondMesh.instanceMatrix.needsUpdate = true;
        if (this.bondMesh.instanceColor) this.bondMesh.instanceColor.needsUpdate = true;
        this.scene.add(this.bondMesh);
      }
    }

    if (showCell && Number.isFinite(extent)) {
      const box = new THREE.BoxGeometry(extent, extent, extent);
      this.cell = new THREE.LineSegments(
        new THREE.EdgesGeometry(box),
        new THREE.LineBasicMaterial({ color: 0x7186b8, transparent: true, opacity: 0.42 }),
      );
      this.scene.add(this.cell);
    }

    if (showVoid && points.length) this.addVoidField(points, extent);
    this.updateSelection();
    if (shouldFrame) this.reset();
    this.requestRender();
  }

  addVoidField(points, extent) {
    const grid = Math.min(12, Math.max(6, Math.ceil(Math.cbrt(points.length)) + 2));
    const bins = Array.from({ length: grid ** 3 }, () => []);
    const cellOf = (point) => point.map((value) =>
      Math.min(grid - 1, Math.floor((((value + extent / 2) % extent + extent) % extent) / extent * grid))
    );
    const indexOf = ([x, y, z]) => (x * grid + y) * grid + z;
    points.forEach((point) => bins[indexOf(cellOf(point))].push(point));
    const voids = [];
    const clearances = [];
    for (let x = 0; x < grid; x += 1) {
      for (let y = 0; y < grid; y += 1) {
        for (let z = 0; z < grid; z += 1) {
          const sample = [x, y, z].map((index) => extent * ((index + 0.5) / grid - 0.5));
          let nearest = Infinity;
          const cell = cellOf(sample);
          for (let dx = -1; dx <= 1; dx += 1) {
            for (let dy = -1; dy <= 1; dy += 1) {
              for (let dz = -1; dz <= 1; dz += 1) {
                const neighbour = [
                  (cell[0] + dx + grid) % grid,
                  (cell[1] + dy + grid) % grid,
                  (cell[2] + dz + grid) % grid,
                ];
                for (const point of bins[indexOf(neighbour)]) {
                  let distanceSquared = 0;
                  for (let axis = 0; axis < 3; axis += 1) {
                    let displacement = sample[axis] - point[axis];
                    displacement -= extent * Math.round(displacement / extent);
                    distanceSquared += displacement * displacement;
                  }
                  nearest = Math.min(nearest, Math.sqrt(distanceSquared));
                }
              }
            }
          }
          if (nearest > 0.72) {
            voids.push(...sample);
            clearances.push(Number.isFinite(nearest) ? nearest : 1.44);
          }
        }
      }
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.Float32BufferAttribute(voids, 3));
    const colors = [];
    const low = new THREE.Color(0x34e7ff);
    const high = new THREE.Color(0xff4fd8);
    for (const clearance of clearances) {
      colors.push(...low.clone().lerp(high, Math.min(1, Math.max(0, (clearance - 0.72) / 0.9))).toArray());
    }
    geometry.setAttribute("color", new THREE.Float32BufferAttribute(colors, 3));
    const material = new THREE.PointsMaterial({
      vertexColors: true,
      size: Math.max(0.08, extent * 0.018),
      transparent: true,
      opacity: 0.82,
      depthWrite: false,
      depthTest: false,
      blending: THREE.AdditiveBlending,
      sizeAttenuation: true,
      fog: false,
      toneMapped: false,
    });
    this.voidPoints = new THREE.Points(geometry, material);
    this.scene.add(this.voidPoints);
  }

  normalizedPointer(event) {
    const bounds = this.canvas.getBoundingClientRect();
    return new THREE.Vector2(
      ((event.clientX - bounds.left) / bounds.width) * 2 - 1,
      -((event.clientY - bounds.top) / bounds.height) * 2 + 1,
    );
  }

  intersect(event) {
    if (!this.atomMesh || this.lastPoints.length > 5000) return null;
    this.raycaster.setFromCamera(this.normalizedPointer(event), this.camera);
    return this.raycaster.intersectObject(this.atomMesh, false)[0] ?? null;
  }

  pick(event) {
    const hit = this.intersect(event);
    this.selected = hit?.instanceId ?? -1;
    this.updateSelection();
    this.onSelect?.(this.selected);
  }

  hover(event) {
    this.canvas.style.cursor = this.intersect(event) ? "pointer" : "grab";
  }

  updateSelection() {
    const point = this.lastPoints[this.selected];
    this.selectionHalo.visible = Boolean(point);
    if (point) this.selectionHalo.position.fromArray(point);
    this.requestRender();
  }

  reset() {
    const distance = Math.max(5, this.lastExtent * 1.65);
    this.camera.position.set(distance, distance * 0.72, distance);
    this.controls.target.set(0, 0, 0);
    this.controls.update();
    this.requestRender();
  }

  resize() {
    const width = Math.max(1, this.canvas.clientWidth);
    const height = Math.max(1, this.canvas.clientHeight);
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    this.requestRender();
  }

  requestRender() {
    if (this.renderQueued) return;
    this.renderQueued = true;
    requestAnimationFrame(() => {
      this.renderQueued = false;
      this.renderer.render(this.scene, this.camera);
    });
  }
}
