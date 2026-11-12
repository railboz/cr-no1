'use strict';

/* ============================================================
   Créno (extension Firefox) — page d'arrière-plan (event page)

   Un clic sur l'icône ouvre Créno dans un onglet. Si l'onglet Créno
   créé lors d'un clic précédent existe encore, il est réactivé et sa
   fenêtre remonte au premier plan ; sinon (onglet fermé, identifiant
   périmé, stockage vide) un nouvel onglet est créé et son identifiant
   est mémorisé.

   Pourquoi un identifiant mémorisé plutôt qu'une recherche par URL :
   filtrer les onglets sur leur URL demande la permission « tabs », qui
   affiche l'avertissement « Lire votre historique de navigation ».
   Créno ne la demande plus ; `tabs.get` et `tabs.update` sur l'identifiant
   connu n'en ont pas besoin.

   Contrainte « event page » (manifest : "persistent": false) :
   aucun travail au chargement — seulement l'enregistrement du
   listener de premier niveau. Toute E/S a lieu dans le handler.
   ============================================================ */

const APP_PAGE = 'index.html';
const TAB_KEY = 'crenoTabId';

/* Zone de stockage de l'identifiant : session d'abord (effacée à la
   fermeture du navigateur, aucune trace durable), repli sur local si
   l'API session manque. */
function tabStore() {
  return (browser.storage && browser.storage.session) || browser.storage.local;
}

/* Identifiant mémorisé, ou null si absent / illisible. */
async function readTabId() {
  const stored = await tabStore().get(TAB_KEY);
  const id = stored ? stored[TAB_KEY] : undefined;
  return typeof id === 'number' ? id : null;
}

async function writeTabId(id) {
  await tabStore().set({ [TAB_KEY]: id });
}

async function forgetTabId() {
  await tabStore().remove(TAB_KEY);
}

/* Réactive l'onglet mémorisé. Lève si l'onglet a disparu (tabs.get). */
async function focusKnownTab(id) {
  const tab = await browser.tabs.get(id);
  await browser.tabs.update(id, { active: true });
  if (typeof tab.windowId === 'number') {
    /* Remonter la fenêtre est un confort : si elle a disparu entre-temps,
       l'onglet est déjà actif, inutile d'en ouvrir un second. */
    try {
      await browser.windows.update(tab.windowId, { focused: true });
    } catch (err) {
      console.warn('Créno : fenêtre non remontée', err);
    }
  }
}

/* Crée l'onglet Créno et mémorise son identifiant (échec de stockage non
   bloquant : le prochain clic rouvrira simplement un onglet). */
async function createTab() {
  const tab = await browser.tabs.create({ url: browser.runtime.getURL(APP_PAGE) });
  if (tab && typeof tab.id === 'number') {
    await writeTabId(tab.id).catch(err => console.warn('Créno : identifiant non mémorisé', err));
  }
  return tab;
}

async function openCreno() {
  const knownId = await readTabId().catch(() => null);
  if (knownId !== null) {
    try {
      await focusKnownTab(knownId);
      return;
    } catch (err) {
      /* Onglet fermé ou identifiant périmé : on repart d'un onglet neuf. */
      await forgetTabId().catch(() => {});
    }
  }
  await createTab();
}

/* Enregistrement unique, au chargement de la page d'arrière-plan.
   Aucune E/S, aucun timer, aucune promesse lancée ici. */
browser.browserAction.onClicked.addListener(() => {
  openCreno().catch(err => console.error('Créno : ouverture impossible', err));
});
