import { LitElement, html } from 'lit';
import type { TemplateResult } from 'lit';
import { version } from 'oxedi-engine';

/** The embeddable EDI 835 viewer; for now it only shows the engine version. */
export class OxediViewer extends LitElement {
  override render(): TemplateResult {
    return html`<footer>oxedi v${version()}</footer>`;
  }
}

// A second load of the bundle must not throw on the already-registered name.
if (!customElements.get('oxedi-viewer')) {
  customElements.define('oxedi-viewer', OxediViewer);
}

declare global {
  interface HTMLElementTagNameMap {
    'oxedi-viewer': OxediViewer;
  }
}
