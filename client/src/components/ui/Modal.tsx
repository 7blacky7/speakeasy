import { createEffect, onCleanup, type JSX } from "solid-js";
import styles from "./Modal.module.css";

interface ModalProps {
  title: string;
  onClose: () => void;
  children: JSX.Element;
  actions?: JSX.Element;
}

export default function Modal(props: ModalProps) {
  const handleOverlayClick = (e: MouseEvent) => {
    if (e.target === e.currentTarget) {
      props.onClose();
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      props.onClose();
    }
  };

  createEffect(() => {
    document.addEventListener("keydown", handleKeyDown);
    onCleanup(() => document.removeEventListener("keydown", handleKeyDown));
  });

  return (
    <div class={styles.overlay} onClick={handleOverlayClick}>
      <div class={styles.dialog}>
        <div class={styles.header}>
          <span class={styles.title}>{props.title}</span>
          <button class={styles.closeBtn} onClick={props.onClose}>
            x
          </button>
        </div>
        <div class={styles.content}>{props.children}</div>
        {props.actions && (
          <div class={styles.actions}>{props.actions}</div>
        )}
      </div>
    </div>
  );
}
