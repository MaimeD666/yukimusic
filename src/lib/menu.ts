export type MenuItem = {
  label: string;
  icon?: string;
  disabled?: boolean;
  separator?: boolean;
  run: () => void | Promise<unknown>;
};
export type Menu = { x: number; y: number; items: MenuItem[] };
