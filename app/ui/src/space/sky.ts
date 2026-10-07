// Space's sky in three.js (docs/SPEC.md 4.3–4.5): the universe tier, with
// every galaxy where the model places it, under a still starfield. Drawn on
// demand, never on a loop, so nothing moves while nothing plays (gate 2.4).
// Text stays flat: labels are DOM elements at each body's projected point
// (4.5), which `project()` gives the view.

import * as THREE from 'three';
import { fnv1a32 } from '../shared/dmath/fnv';
import type { Galaxy } from './model/catalogue';
import { GALAXY_GLOW, placeGalaxies, universeBox } from './model/orbits';

/** The hub's default pitch, 0.62 rad, and its limits (4.4). */
export const PITCH = 0.62;
/** Each zoom button steps by this (4.4). */
export const ZOOM_STEP = 1.55;
const FOV = 40;
const STARS = 4000;

export interface Projected {
  id: number;
  name: string;
  x: number;
  y: number;
  visible: boolean;
}

export interface Sky {
  /** Draws one frame. */
  render(): void;
  resize(width: number, height: number): void;
  /** Steps the camera in (factor > 1) or out. */
  zoom(factor: number): void;
  /** Frames every galaxy again. */
  fit(): void;
  /** Where each galaxy sits on screen, for its flat label. */
  project(): Projected[];
  /** The galaxy under a point, within 28 pt of its sun. */
  pick(x: number, y: number): Galaxy | null;
  dispose(): void;
}

/** A soft radial glow, drawn once on a canvas: a galaxy seen from afar. */
function glowTexture(): THREE.Texture {
  const size = 128;
  const c = document.createElement('canvas');
  c.width = c.height = size;
  const g = c.getContext('2d')!;
  const grad = g.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
  grad.addColorStop(0, 'rgba(255, 248, 230, 1)');
  grad.addColorStop(0.12, 'rgba(255, 216, 154, 0.9)');
  grad.addColorStop(0.35, 'rgba(240, 162, 60, 0.28)');
  grad.addColorStop(1, 'rgba(41, 70, 255, 0)');
  g.fillStyle = grad;
  g.fillRect(0, 0, size, size);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

/** The same stars on every machine: positions from a stable hash, never Math.random. */
function starfield(radius: number): THREE.Points {
  const positions = new Float32Array(STARS * 3);
  const colours = new Float32Array(STARS * 3);
  // Murmur3's finalizer over an integer: every input bit reaches every
  // output bit, so neighbouring stars land far apart. (FNV over strings that
  // differ only in their last character keeps the coordinates in lockstep,
  // which draws every star on one spiral.)
  const mix = (n: number) => {
    let z = Math.imul(n ^ (n >>> 16), 0x85ebca6b);
    z = Math.imul(z ^ (z >>> 13), 0xc2b2ae35);
    return ((z ^ (z >>> 16)) >>> 0) / 4294967296;
  };
  const unit = (i: number, salt: string) => mix(i * 3 + 0x9e3779b9 + (salt === 'u' ? 1 : salt === 't' ? 2 : 3));
  for (let i = 0; i < STARS; i++) {
    const u = unit(i, 'u') * 2 - 1;
    const theta = unit(i, 't') * Math.PI * 2;
    const s = Math.sqrt(1 - u * u);
    positions.set([radius * s * Math.cos(theta), radius * u, radius * s * Math.sin(theta)], i * 3);
    const b = 0.35 + unit(i, 'b') * 0.65;
    colours.set([(0xdd / 255) * b, (0xe4 / 255) * b, (0xfb / 255) * b], i * 3);
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute('color', new THREE.BufferAttribute(colours, 3));
  const material = new THREE.PointsMaterial({ size: 2, sizeAttenuation: false, vertexColors: true, depthWrite: false });
  return new THREE.Points(geometry, material);
}

export function createSky(canvas: HTMLCanvasElement, galaxies: Galaxy[]): Sky {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.setClearColor(0x070a18, 1);

  const scene = new THREE.Scene();
  const placed = placeGalaxies(galaxies);
  const box = universeBox(placed);
  const cx = box.x + box.w / 2;
  const cz = box.y + box.h / 2;
  const span = Math.max(box.w, box.h);

  scene.add(starfield(span * 8));

  const glow = glowTexture();
  const byId = new Map(galaxies.map((g) => [g.id, g]));
  const suns: { galaxy: Galaxy; at: THREE.Vector3 }[] = [];
  for (const p of placed) {
    const galaxy = byId.get(p.id);
    if (!galaxy) continue;
    const at = new THREE.Vector3(p.x - cx, 0, p.y - cz);
    const halo = new THREE.Sprite(new THREE.SpriteMaterial({ map: glow, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending }));
    halo.position.copy(at);
    // At the universe tier a galaxy is a haloed dot (4.4), drawn larger than
    // its glow radius so it reads from across everyone's sky.
    halo.scale.set(GALAXY_GLOW * 5, GALAXY_GLOW * 5, 1);
    scene.add(halo);
    // Each solar system as a faint point around its galaxy's sun.
    const dots = new Float32Array(galaxy.systems.length * 3);
    galaxy.systems.forEach((s, i) => {
      const a = (fnv1a32(`${galaxy.slug}-${s.slug}`) / 0xffffffff) * Math.PI * 2;
      const r = GALAXY_GLOW * (0.35 + 0.5 * ((i + 1) / (galaxy.systems.length + 1)));
      dots.set([at.x + Math.cos(a) * r, 0, at.z + Math.sin(a) * r], i * 3);
    });
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(dots, 3));
    scene.add(new THREE.Points(geometry, new THREE.PointsMaterial({ size: 3, sizeAttenuation: false, color: 0xf4efe6, transparent: true, opacity: 0.7 })));
    suns.push({ galaxy, at });
  }

  const camera = new THREE.PerspectiveCamera(FOV, 1, 10, span * 40);
  const fitDistance = () => (span / 2 / Math.tan((FOV * Math.PI) / 360)) * 1.15;
  let distance = fitDistance();
  let width = 1;
  let height = 1;

  function place() {
    camera.position.set(0, distance * Math.sin(PITCH), distance * Math.cos(PITCH));
    camera.lookAt(0, 0, 0);
    camera.updateMatrixWorld();
  }

  function screenOf(v: THREE.Vector3): { x: number; y: number; visible: boolean } {
    const p = v.clone().project(camera);
    return { x: ((p.x + 1) / 2) * width, y: ((1 - p.y) / 2) * height, visible: p.z > -1 && p.z < 1 };
  }

  const sky: Sky = {
    render() {
      place();
      renderer.render(scene, camera);
    },
    resize(w, h) {
      width = Math.max(1, w);
      height = Math.max(1, h);
      renderer.setSize(width, height, false);
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
      sky.render();
    },
    zoom(factor) {
      distance = Math.min(fitDistance() * 4, Math.max(GALAXY_GLOW * 1.2, distance / factor));
      sky.render();
    },
    fit() {
      distance = fitDistance();
      sky.render();
    },
    project() {
      place();
      return suns.map(({ galaxy, at }) => ({ id: galaxy.id, name: galaxy.displayName, ...screenOf(at) }));
    },
    pick(x, y) {
      let best: Galaxy | null = null;
      let bestD = 28;
      for (const { galaxy, at } of suns) {
        const s = screenOf(at);
        const d = Math.hypot(s.x - x, s.y - y);
        if (s.visible && d <= bestD) {
          best = galaxy;
          bestD = d;
        }
      }
      return best;
    },
    dispose() {
      glow.dispose();
      scene.traverse((o) => {
        const mesh = o as THREE.Mesh;
        mesh.geometry?.dispose();
        const m = mesh.material as THREE.Material | THREE.Material[] | undefined;
        if (Array.isArray(m)) m.forEach((x) => x.dispose());
        else m?.dispose();
      });
      renderer.dispose();
    },
  };
  return sky;
}
