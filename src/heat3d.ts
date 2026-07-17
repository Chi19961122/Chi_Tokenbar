import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { HeatGrid } from "./analytics";
import { heatBarHeight } from "./analytics";
import { fmtTokens } from "./format";

interface SceneInstance {
  container: HTMLElement;
  renderer: THREE.WebGLRenderer;
  scene: THREE.Scene;
  camera: THREE.PerspectiveCamera;
  controls: OrbitControls;
  raycaster: THREE.Raycaster;
  pointer: THREE.Vector2;
  geometry: THREE.BoxGeometry;
  meshes: THREE.Mesh<THREE.BoxGeometry, THREE.MeshStandardMaterial>[];
  tooltip: HTMLDivElement;
  resizeObserver: ResizeObserver;
  hovered: THREE.Mesh<THREE.BoxGeometry, THREE.MeshStandardMaterial> | null;
  signature: string;
  onPointerMove: (event: PointerEvent) => void;
  onPointerLeave: () => void;
}

let active: SceneInstance | null = null;

function signature(grid: HeatGrid, totals: Record<string, number>): string {
  return grid.cells.map((cell) => `${cell.date}:${cell.intensity}:${totals[cell.date] ?? 0}`).join("|");
}

function level(intensity: number): number {
  return intensity === 0 ? 0 : Math.min(4, Math.ceil(Math.max(0, intensity) * 4));
}

function heatColors(container: HTMLElement): Array<{ color: THREE.Color; opacity: number }> {
  const probe = document.createElement("i");
  probe.className = "hm-cell";
  probe.hidden = true;
  container.appendChild(probe);
  const colors = [0, 1, 2, 3, 4].map((n) => {
    probe.className = `hm-cell hm-l${n}`;
    const css = getComputedStyle(probe).backgroundColor;
    const match = css.match(/[\d.]+/g)?.map(Number) ?? [];
    return match.length >= 3
      ? { color: new THREE.Color(match[0] / 255, match[1] / 255, match[2] / 255), opacity: match[3] ?? 1 }
      : { color: new THREE.Color("#2fa87e"), opacity: 1 };
  });
  probe.remove();
  return colors;
}

function render(instance: SceneInstance): void {
  instance.renderer.render(instance.scene, instance.camera);
}

function size(instance: SceneInstance): void {
  const width = Math.max(1, instance.container.clientWidth);
  const height = Math.max(1, Math.min(160, instance.container.clientHeight || 150));
  instance.renderer.setSize(width, height, false);
  instance.camera.aspect = width / height;
  instance.camera.updateProjectionMatrix();
  render(instance);
}

function updateMeshes(instance: SceneInstance, grid: HeatGrid, totals: Record<string, number>): void {
  const colors = heatColors(instance.container);
  grid.cells.forEach((cell, index) => {
    const mesh = instance.meshes[index];
    const height = heatBarHeight(cell.intensity);
    mesh.position.set(cell.weekCol - (grid.weeks - 1) / 2, height / 2, cell.weekdayRow - 3);
    mesh.scale.set(0.82, height, 0.82);
    const ramp = colors[level(cell.intensity)];
    mesh.material.color.copy(ramp.color);
    mesh.material.opacity = ramp.opacity;
    mesh.material.emissive.set(0x000000);
    mesh.userData.label = `${cell.date} · ${fmtTokens(totals[cell.date] ?? 0)}`;
  });
  instance.signature = signature(grid, totals);
  render(instance);
}

function attach(instance: SceneInstance, container: HTMLElement): void {
  if (instance.container === container && instance.renderer.domElement.parentElement === container) return;
  instance.container.removeEventListener("pointermove", instance.onPointerMove);
  instance.container.removeEventListener("pointerleave", instance.onPointerLeave);
  instance.resizeObserver.disconnect();
  instance.container = container;
  container.append(instance.renderer.domElement, instance.tooltip);
  container.addEventListener("pointermove", instance.onPointerMove);
  container.addEventListener("pointerleave", instance.onPointerLeave);
  instance.resizeObserver.observe(container);
  size(instance);
}

/** Mounts or updates the singleton analytics scene. False means silent 2D fallback. */
export function mountHeat3d(
  container: HTMLElement,
  grid: HeatGrid,
  totals: Record<string, number>,
): boolean {
  if (grid.cells.length === 0) {
    disposeHeat3d();
    return false;
  }
  const nextSignature = signature(grid, totals);
  if (active) {
    attach(active, container);
    if (active.signature !== nextSignature && active.meshes.length === grid.cells.length) {
      updateMeshes(active, grid, totals);
    } else if (active.signature !== nextSignature) {
      disposeHeat3d();
    } else {
      render(active);
      return true;
    }
    if (active) return true;
  }

  let renderer: THREE.WebGLRenderer | null = null;
  try {
    const canvas = document.createElement("canvas");
    const context = canvas.getContext("webgl2", { antialias: true, alpha: true })
      ?? canvas.getContext("webgl", { antialias: true, alpha: true });
    if (!context) return false;
    renderer = new THREE.WebGLRenderer({ canvas, context, antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(38, 2, 0.1, 100);
    const span = Math.max(grid.weeks, 7);
    camera.position.set(span * 0.72, 8.2, span * 0.72);

    const controls = new OrbitControls(camera, canvas);
    controls.enableDamping = false;
    controls.enablePan = false;
    controls.minDistance = 7;
    controls.maxDistance = Math.max(16, span * 2.2);
    controls.target.set(0, 0.45, 0);
    controls.update();

    scene.add(new THREE.AmbientLight(0xffffff, 1.5));
    const sun = new THREE.DirectionalLight(0xffffff, 2.1);
    sun.position.set(5, 10, 7);
    scene.add(sun);

    const geometry = new THREE.BoxGeometry(1, 1, 1);
    const colors = heatColors(container);
    const meshes = grid.cells.map((cell) => {
      const ramp = colors[level(cell.intensity)];
      const material = new THREE.MeshStandardMaterial({
        color: ramp.color,
        opacity: ramp.opacity,
        transparent: ramp.opacity < 1,
        roughness: 0.72,
      });
      const mesh = new THREE.Mesh(geometry, material);
      const height = heatBarHeight(cell.intensity);
      mesh.position.set(cell.weekCol - (grid.weeks - 1) / 2, height / 2, cell.weekdayRow - 3);
      mesh.scale.set(0.82, height, 0.82);
      mesh.userData.label = `${cell.date} · ${fmtTokens(totals[cell.date] ?? 0)}`;
      scene.add(mesh);
      return mesh;
    });

    const tooltip = document.createElement("div");
    tooltip.className = "heat3d-tooltip";
    tooltip.hidden = true;
    container.append(canvas, tooltip);

    const instance = {} as SceneInstance;
    Object.assign(instance, {
      container, renderer, scene, camera, controls,
      raycaster: new THREE.Raycaster(), pointer: new THREE.Vector2(), geometry, meshes, tooltip,
      hovered: null, signature: nextSignature,
    });
    instance.onPointerMove = (event: PointerEvent) => {
      const rect = renderer!.domElement.getBoundingClientRect();
      instance.pointer.set(((event.clientX - rect.left) / rect.width) * 2 - 1, -((event.clientY - rect.top) / rect.height) * 2 + 1);
      instance.raycaster.setFromCamera(instance.pointer, camera);
      const hit = instance.raycaster.intersectObjects(meshes, false)[0]?.object as SceneInstance["hovered"];
      if (hit !== instance.hovered) {
        instance.hovered?.material.emissive.set(0x000000);
        instance.hovered = hit ?? null;
        instance.hovered?.material.emissive.set(0x26352f);
        render(instance);
      }
      tooltip.hidden = !instance.hovered;
      if (instance.hovered) {
        tooltip.textContent = String(instance.hovered.userData.label);
        tooltip.style.transform = `translate(${event.clientX - rect.left + 10}px, ${event.clientY - rect.top + 10}px)`;
      }
    };
    instance.onPointerLeave = () => {
      if (instance.hovered) {
        instance.hovered.material.emissive.set(0x000000);
        instance.hovered = null;
        render(instance);
      }
      tooltip.hidden = true;
    };
    instance.resizeObserver = new ResizeObserver(() => size(instance));
    controls.addEventListener("change", () => render(instance));
    container.addEventListener("pointermove", instance.onPointerMove);
    container.addEventListener("pointerleave", instance.onPointerLeave);
    instance.resizeObserver.observe(container);
    active = instance;
    size(instance);
    return true;
  } catch {
    renderer?.dispose();
    renderer?.forceContextLoss();
    return false;
  }
}

/** Releases every GPU/DOM/control resource owned by the current scene. */
export function disposeHeat3d(): void {
  const instance = active;
  if (!instance) return;
  active = null;
  instance.resizeObserver.disconnect();
  instance.container.removeEventListener("pointermove", instance.onPointerMove);
  instance.container.removeEventListener("pointerleave", instance.onPointerLeave);
  instance.controls.dispose();
  instance.geometry.dispose();
  for (const mesh of instance.meshes) mesh.material.dispose();
  instance.renderer.renderLists.dispose();
  instance.renderer.dispose();
  instance.renderer.forceContextLoss();
  instance.renderer.domElement.remove();
  instance.tooltip.remove();
  instance.scene.clear();
}
