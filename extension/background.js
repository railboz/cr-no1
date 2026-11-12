'use strict';

/* ============================================================
   Créno (extension Firefox) — page d'arrière-plan (event page)
   Un clic sur l'icône ouvre Créno dans un onglet. Si un onglet
   Créno est déjà ouvert, il est réutilisé et son onglet passe
   au premier plan.

   Contrainte « event page » (manifest : "persistent": false) :
   aucun travail au chargement — seulement l'enregistrement du
   listener de premier niveau. Firefox réveille la page au clic.
   ============================================================ */

const APP_PAGE = 'index.html';

/* Réutilise un onglet Créno existant, sinon en crée un. */
async function openCreno() {
  const url = browser.runtime.getURL(APP_PAGE);

  /* `tabs.query({ url })` exige la permission "tabs" (ou une
     permission d'hôte correspondante, impossible pour moz-extension://). */
  const tabs = await browser.tabs.query({ url });
  if (tabs.length) {
    const tab = tabs[tabs.length - 1];
    await browser.tabs.update(tab.id, { active: true });
    /* Remonte la fenêtre de l'onglet, s'il n'est pas déjà au premier plan. */
    if (typeof tab.windowId === 'number') {
      await browser.windows.update(tab.windowId, { focused: true });
    }
    return;
  }

  await browser.tabs.create({ url });
}

/* Enregistrement unique, au chargement de la page d'arrière-plan.
   Aucune E/S, aucun timer, aucune promesse lancée ici. */
browser.browserAction.onClicked.addListener(() => {
  openCreno().catch(err => console.error('Créno : ouverture impossible', err));
});
