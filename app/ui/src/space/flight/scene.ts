// The sky you fly through, in three.js (docs/SPACE.md 1: "3D space, not 2D
// space with depth drawn on"). You are the camera. Far stars stay at
// infinity; dust near you streams past as you move, which is how speed is
// felt with nothing else nearby; every body is a sphere of glass around the
// page at its core. Text stays flat: names are DOM elements the view places
// from `sight()`.

import * as THREE from 'three';
import { type Body, type Pilot, type View, FOV, chance, faceOf, sight } from './model';

export interface Sky {
  resize(width: number, height: number): void;
  /** Draws one frame from where you are. `live` is the body whose page is showing, which then needs no screen drawn for it. */
  draw(pilot: Pilot, seconds: number, live: string | null): void;
  /** The body under a point of the view, or null. */
  pick(pilot: Pilot, x: number, y: number): Body | null;
  dispose(): void;
}

const STARS = 7000;
const DUST = 700;
/** Dust comes in three sizes of cloud, so something streams past at any speed. */
const DUST_SPANS = [260, 2600, 26000];

/** Murmur3's finalizer: neighbouring numbers land far apart, and the same on every machine. */
function mix(n: number): number {
  let z = Math.imul(n ^ (n >>> 16), 0x85ebca6b);
  z = Math.imul(z ^ (z >>> 13), 0xc2b2ae35);
  return ((z ^ (z >>> 16)) >>> 0) / 4294967296;
}

function stars(): THREE.Points {
  const positions = new Float32Array(STARS * 3);
  const colours = new Float32Array(STARS * 3);
  for (let i = 0; i < STARS; i++) {
    const u = mix(i * 4 + 1) * 2 - 1;
    const theta = mix(i * 4 + 2) * Math.PI * 2;
    const s = Math.sqrt(1 - u * u);
    positions.set([s * Math.cos(theta), u, s * Math.sin(theta)], i * 3);
    // Most stars are faint; a few are bright, and some lean warm or cool.
    const bright = 0.18 + Math.pow(mix(i * 4 + 3), 3.2) * 0.82;
    const warm = mix(i * 4 + 4);
    colours.set([bright * (0.86 + warm * 0.14), bright * 0.9, bright * (1 - warm * 0.16)], i * 3);
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute('color', new THREE.BufferAttribute(colours, 3));
  const material = new THREE.PointsMaterial({ size: 1.8, sizeAttenuation: false, vertexColors: true, depthWrite: false, depthTest: false });
  const points = new THREE.Points(geometry, material);
  points.frustumCulled = false;
  points.renderOrder = -2;
  return points;
}

interface Dust {
  points: THREE.Points;
  seeds: Float32Array;
  span: number;
}

function dust(span: number, salt: number): Dust {
  const seeds = new Float32Array(DUST * 3);
  for (let i = 0; i < DUST * 3; i++) seeds[i] = mix(i + salt * 7919) * span;
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(DUST * 3), 3));
  const material = new THREE.PointsMaterial({
    size: 1.4,
    sizeAttenuation: false,
    color: 0x9fb0e8,
    transparent: true,
    opacity: 0.42,
    depthWrite: false,
  });
  const points = new THREE.Points(geometry, material);
  points.frustumCulled = false;
  points.renderOrder = -1;
  return { points, seeds, span };
}

/** Each grain stays put in space; the cloud is the grains within half a span of you, wrapped. */
function drift(d: Dust, at: THREE.Vector3): void {
  const position = d.points.geometry.getAttribute('position') as THREE.BufferAttribute;
  const out = position.array as Float32Array;
  const { seeds, span } = d;
  const p = [at.x, at.y, at.z];
  for (let i = 0; i < seeds.length; i++) {
    const c = p[i % 3];
    const rel = (((seeds[i] - c) % span) + span) % span;
    out[i] = c + rel - span / 2;
  }
  position.needsUpdate = true;
}

/** A soft round glow, drawn once: how a body reads from far away. */
function glowTexture(): THREE.Texture {
  const size = 128;
  const c = document.createElement('canvas');
  c.width = c.height = size;
  const g = c.getContext('2d');
  if (g) {
    const grad = g.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
    grad.addColorStop(0, 'rgba(255, 255, 255, 1)');
    grad.addColorStop(0.18, 'rgba(255, 255, 255, 0.55)');
    grad.addColorStop(0.5, 'rgba(255, 255, 255, 0.12)');
    grad.addColorStop(1, 'rgba(255, 255, 255, 0)');
    g.fillStyle = grad;
    g.fillRect(0, 0, size, size);
  }
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

/** A body's colour from its link: the same link is the same colour everywhere. Big bodies burn warmer and brighter. */
export function colourOf(body: Body): THREE.Color {
  const hue = chance(body.url, 'hue');
  const sun = Math.min(1, Math.max(0, (body.magnitude - 5) / 5));
  return new THREE.Color().setHSL(hue, 0.55 + 0.25 * sun, 0.58 + 0.1 * sun);
}

// Glass: almost clear seen straight on, bright at the rim, so the page at
// the core shows through and the sphere still reads as a sphere.
const GLASS_VERTEX = `
  varying vec3 vNormal;
  varying vec3 vView;
  void main() {
    vec4 mv = modelViewMatrix * vec4(position, 1.0);
    vNormal = normalize(normalMatrix * normal);
    vView = normalize(-mv.xyz);
    gl_Position = projectionMatrix * mv;
  }
`;
const GLASS_FRAGMENT = `
  uniform vec3 tint;
  uniform float glow;
  varying vec3 vNormal;
  varying vec3 vView;
  void main() {
    float facing = abs(dot(normalize(vNormal), normalize(vView)));
    float rim = pow(1.0 - facing, 2.4);
    float alpha = 0.05 + glow * 0.07 + rim * (0.55 + glow * 0.35);
    gl_FragColor = vec4(tint * (0.55 + rim * 0.9 + glow * 0.35), alpha);
  }
`;

interface Drawn {
  body: Body;
  face: THREE.Mesh;
  edge: THREE.LineSegments;
  spark: THREE.Sprite;
}

export function createSky(canvas: HTMLCanvasElement, bodies: readonly Body[]): Sky {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.setClearColor(0x030510, 1);

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(FOV, 1, 0.5, 400000);
  const view: View = { width: 1, height: 1 };

  const far = stars();
  scene.add(far);
  const clouds = DUST_SPANS.map((span, i) => dust(span, i + 1));
  for (const c of clouds) scene.add(c.points);

  const glow = glowTexture();
  const plane = new THREE.PlaneGeometry(1, 1);
  const outline = new THREE.EdgesGeometry(plane);
  const drawn: Drawn[] = [];

  for (const body of bodies) {
    const tint = colourOf(body);
    const sun = Math.min(1, Math.max(0, (body.magnitude - 5) / 5));

    const glass = new THREE.Mesh(
      new THREE.SphereGeometry(body.radius, 64, 40),
      new THREE.ShaderMaterial({
        uniforms: { tint: { value: tint }, glow: { value: sun } },
        vertexShader: GLASS_VERTEX,
        fragmentShader: GLASS_FRAGMENT,
        transparent: true,
        depthWrite: false,
        side: THREE.DoubleSide,
      }),
    );
    glass.position.copy(body.at);
    glass.renderOrder = 2;
    scene.add(glass);

    // The light it gives off, which is all you see of it from far away.
    const halo = new THREE.Sprite(
      new THREE.SpriteMaterial({ map: glow, color: tint, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending, opacity: 0.35 + 0.45 * sun }),
    );
    halo.position.copy(body.at);
    halo.scale.setScalar(body.radius * (2.6 + 2.2 * sun));
    halo.renderOrder = 1;
    scene.add(halo);

    // And a point of that light that never shrinks to nothing, so no link is lost in the dark.
    const spark = new THREE.Sprite(
      new THREE.SpriteMaterial({ map: glow, color: tint, transparent: true, depthWrite: false, depthTest: false, blending: THREE.AdditiveBlending, sizeAttenuation: false }),
    );
    spark.position.copy(body.at);
    spark.renderOrder = 0;
    scene.add(spark);

    // The page at its core: a dark screen until the live page is laid over it.
    const face = new THREE.Mesh(plane, new THREE.MeshBasicMaterial({ color: 0x0a0f24, transparent: true, opacity: 0.9, depthWrite: false }));
    face.position.copy(body.at);
    face.renderOrder = 3;
    scene.add(face);
    const edge = new THREE.LineSegments(outline, new THREE.LineBasicMaterial({ color: tint, transparent: true, opacity: 0.85, depthWrite: false }));
    edge.position.copy(body.at);
    edge.renderOrder = 4;
    scene.add(edge);

    drawn.push({ body, face, edge, spark });
  }

  return {
    resize(w, h) {
      view.width = Math.max(1, w);
      view.height = Math.max(1, h);
      renderer.setSize(view.width, view.height, false);
      camera.aspect = view.width / view.height;
      camera.updateProjectionMatrix();
    },
    draw(pilot, _seconds, live) {
      camera.position.copy(pilot.at);
      camera.quaternion.copy(pilot.facing);
      camera.updateMatrixWorld();
      far.position.copy(pilot.at);
      far.scale.setScalar(200000);
      for (const c of clouds) drift(c, pilot.at);
      for (const d of drawn) {
        const size = faceOf(d.body, view);
        // The page always faces you squarely, so it is a plain rectangle on the screen.
        d.face.quaternion.copy(pilot.facing);
        d.edge.quaternion.copy(pilot.facing);
        d.face.scale.set(size.width, size.height, 1);
        d.edge.scale.set(size.width, size.height, 1);
        const s = sight(pilot, d.body, view);
        // Too small to be a screen, it is only a light.
        const screen = !!s && s.rect.height >= 14;
        d.face.visible = screen && live !== d.body.id;
        d.edge.visible = screen;
        // The point of light: a few points across, more for a bigger body, fading once the sphere itself is big.
        const points = 5 + d.body.magnitude * 1.1;
        const fade = s ? Math.min(1, Math.max(0, 1.4 - s.radius / (points * 1.5))) : 0;
        d.spark.visible = fade > 0.01;
        (d.spark.material as THREE.SpriteMaterial).opacity = fade;
        d.spark.scale.setScalar((points * 2 * Math.tan((FOV * Math.PI) / 360) * 2) / view.height);
      }
      renderer.render(scene, camera);
    },
    pick(pilot, x, y) {
      let best: Body | null = null;
      let nearest = Infinity;
      for (const { body } of drawn) {
        const s = sight(pilot, body, view);
        if (!s) continue;
        const cx = s.rect.x + s.rect.width / 2;
        const cy = s.rect.y + s.rect.height / 2;
        // A far body is a point of light; it is still something to press.
        const reach = Math.max(s.radius, 16);
        if (Math.hypot(x - cx, y - cy) <= reach && s.depth < nearest) {
          best = body;
          nearest = s.depth;
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
}
