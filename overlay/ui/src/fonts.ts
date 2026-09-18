import "./fonts.css";

const SYSTEM = 'Arial, ui-sans-serif, system-ui, sans-serif';

export interface FontChoice {
  name: string;
  stack: string; // value for font-family / the --pd-font var
}

export const FONTS: FontChoice[] = [
  { name: "System", stack: SYSTEM },
  { name: "Inter", stack: `'Inter', ${SYSTEM}` },
  { name: "Manrope", stack: `'Manrope', ${SYSTEM}` },
  { name: "Rubik", stack: `'Rubik', ${SYSTEM}` },
  { name: "Exo 2", stack: `'Exo 2', ${SYSTEM}` },
];

export const DEFAULT_FONT = FONTS[0];
