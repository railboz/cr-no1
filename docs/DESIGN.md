# Créno — Design system « Timetable feutré »

> **Document de référence.** Tout agent qui touche le front (CSS, JS, HTML) le lit avant d'écrire.
> Il décrit l'état **cible**. L'ancien `styles.css` (654 lignes) est remplacé par `styles/*.css`.
>
> **Règle d'or** : en cas de doute, ce document gagne sur l'ancien `styles.css`.
> Toute exception doit être écrite ici, pas dans un commentaire de fichier.

---

## Sommaire

1. [Identité et palette](#1-identité-et-palette)
2. [Tokens et échelles](#2-tokens-et-échelles)
3. [Contrat JS → CSS (`--c` / `--v`)](#3-contrat-js--css)
4. [Catalogue de composants](#4-catalogue-de-composants)
5. [Composition écran par écran](#5-composition-écran-par-écran)
6. [Vocabulaire de classes](#6-vocabulaire-de-classes)
7. [Mapping des tokens legacy](#7-mapping-des-tokens-legacy)
8. [Accessibilité](#8-accessibilité)
9. [Pièges d'implémentation](#9-pièges-dimplémentation)
10. [Ordre de chargement des CSS](#10-ordre-de-chargement-des-css)
11. [Checklist de recette](#11-checklist-de-recette)

---

## 1. Identité et palette

### 1.1 Ce qui ne change pas

| Élément | Valeur | Rôle |
|---|---|---|
| Canvas | `#0a0e15` | nuit bleutée, dark-only |
| Surfaces | `#121722` → `#171d2a` → `#1e2634` | 3 paliers, écart volontairement faible |
| Accent | `#34d399` → `#10b981` | **action, argent, succès** — une seule couleur d'action |
| Info | `#5b9cff` | **« aujourd'hui » / présent** — jamais décoratif |
| Danger | `#f87171` | erreur, destructif |
| Avertissement | `#fbbf24` | avertissement de saisie / relais |
| Absence | `#a78bfa` | famille absence (défaut) |
| Blocs shift | couleur établissement + encre `#08110c` | signature visuelle n°1 |

**Aucune variante claire. Aucune police distante. Aucun framework. Aucune ressource réseau.**

### 1.2 Contrastes mesurés (WCAG 2.1, sRGB)

| Combinaison | Ratio | Usage |
|---|---|---|
| `--c-text` / `--c-s1` | **15.40** | texte courant |
| `--c-text-2` / `--c-s1` | **7.36** | texte secondaire |
| `--c-text-3` / `--c-s1` | **5.75** | texte tertiaire (AA, 12 px) |
| `--c-text-3` / `--c-s2` / `--c-s3` | **5.41** / **4.87** | idem sur surfaces élevées |
| `--c-text-off` / `--c-s1` | 3.79 | **décoratif uniquement** |
| `--c-accent` / `--c-s1` | **9.33** | libellés accent, montant |
| `--c-info` / `--c-s1` | **6.53** | « aujourd'hui » |
| `--c-danger` / `--c-s1` | **6.48** | erreur |
| `--c-on-accent` / `--c-accent` | **8.66** | texte du CTA primaire |
| `--c-on-info` / `--c-info` | **7.04** | numéro du jour |
| `--c-shift-ink` / les 8 couleurs `PALETTE` | **6.93 → 11.48** | blocs de shift |
| `--c-border-field` / `--c-s1` `/ --c-s2` | **3.79** / **3.57** | bord de champ (min 3:1) |
| anneau de focus `--c-accent` / `--c-s2` | **8.77** | focus visible (min 3:1) |

`PALETTE` (JS, fermée, 8 valeurs) : `#34d399 #60a5fa #f472b6 #fbbf24 #a78bfa #f87171 #2dd4bf #e879f9`.
L'encre `#08110c` est validée sur les 8 : toujours du texte sombre sur un bloc coloré, **jamais de blanc**.

---

## 2. Tokens et échelles

Source de vérité : **`styles/tokens.css`**. Ne jamais redéfinir un token ailleurs.

### 2.1 Typographie

| Rôle | Token | Taille / graisse / interligne | Couleur |
|---|---|---|---|
| Titre de page (`h2`) | `--fs-7` → `--fs-8` ≥1024 | 22/28 · 700 · tight | `--c-text` |
| Titre de dialogue (`h3`) | `--fs-6` | 18 · 700 · tight | `--c-text` |
| Chiffre héros (salaire) | `--fs-8` / `--fs-9` ≥1024 | 28/36 · 700 · tabular | `--c-accent` |
| Valeur de statistique | `--fs-6` | 18 · 700 · tabular | `--c-text` |
| Corps | `--fs-4` (16 px < 720 px, cf. `base.css`) | 14/16 · 400 · base | `--c-text` |
| UI dense (grille) | `--fs-3` | 13 · 500 · snug | `--c-text` / `--c-text-2` |
| Méta (durée, taux) | `--fs-2` | 12 · 500 · snug · tabular | `--c-text-2` |
| Libellé de section | `--fs-1` | 11 · 600 · `--tr-eyebrow` uppercase | `--c-text-3` |
| Micro-label sur bloc | `--fs-1` | 11 · 600 · `.06em` uppercase | `currentColor` |

**Interdits** : `font-weight: 800` (faux-gras sur `system-ui`), tailles hors échelle, `letter-spacing` positif hors libellés uppercase.
**Obligatoire** : `font-variant-numeric: tabular-nums` sur toute heure, durée, montant, numéro de semaine (classe `.num` dans `utilities.css`).

### 2.2 Espacement (base 4)

`--sp-1` 2 · `--sp-2` 4 · `--sp-3` 6 · `--sp-4` 8 · `--sp-5` 12 · `--sp-6` 16 · `--sp-7` 20 · `--sp-8` 24 · `--sp-9` 32 · `--sp-10` 40 · `--sp-11` 48.

| Contexte | 375 px | 768 px | 1024 px | 1440 px |
|---|---|---|---|---|
| Marge latérale de page | `--sp-5` | `--sp-6` | `--sp-8` | `--sp-9` |
| Padding de carte | `--sp-5` | `--sp-6` | `--sp-7` | `--sp-7` |
| Gouttière de grille | `--sp-4` | `--sp-4` | `--sp-5` | `--sp-5` |
| Padding de cellule de semaine | `--sp-2` | `--sp-2` | `--sp-3` | `--sp-3` |

### 2.3 Rayons

| Token | Valeur | Usage exclusif |
|---|---|---|
| `--r-sm` | 8 px | bloc `.shift`, `.absence`, petit élément |
| `--r-md` | 10 px | `.btn`, champ, `.icon-btn`, chip carré |
| `--r-lg` | 14 px | `.panel`, `.form-card`, `.summarybar`, `.md-est` |
| `--r-xl` | 18 px | `dialog` |
| `--r-pill` | 999 px | `.chip`, `.week-num`, `.c-pill`, pistes |

### 2.4 Élévations

| Token | Valeur | Usage exclusif |
|---|---|---|
| — | aucune | **défaut** : la séparation se fait par surface + filet |
| `--sh-pop` | `0 10px 30px rgba(0,0,0,.45)` | `.dropdown`, `.toast` |
| `--sh-sheet` | `0 -10px 40px rgba(0,0,0,.5)` | `dialog` en bottom-sheet |
| `--sh-accent` | `0 6px 20px rgba(52,211,153,.22)` | `.btn.primary`, `#dictaBtn` |

Les halos permanents sur les surfaces non flottantes (logo, badge « aujourd'hui », cartes) sont **supprimés**.

### 2.5 Focus

```css
/* utilities.css — global, déjà écrit */
:where(a, button, input, select, textarea, summary, [tabindex]:not([tabindex="-1"])):focus-visible{
  outline: var(--focus-w) solid var(--c-accent);
  outline-offset: var(--focus-off);
  border-radius: inherit;
}
```

Sur aplat de couleur (`.shift`, `.md-shift`, `.absence`, `.md-absence`) : liseré canvas + halo accent (règle `utilities.css`, contractuelle).
**`outline: none` est interdit partout.** Aucun `:focus` sans `:focus-visible`.

### 2.6 Breakpoints

| Nom | min-width | Bascule |
|---|---|---|
| `phone` | 0 | nav basse, vue jour, bottom-sheet, rail masqué |
| `md` | **720** | fin du contrat téléphone : nav haute, grille desktop, dialogues centrés |
| `lg` | 900 | rail + grille en 2 colonnes, formulaires 2 colonnes |
| `xl` | 1024 | rail vertical, **grille sans scroll horizontal** |
| `2xl` | 1440 | marges 32, cellules à l'aise |

⚠️ **720 est un contrat** : `#planning-layout` et `#mobileDayView` sont **rendus en permanence** et le basculement se fait uniquement en CSS. Déplacer le seuil d'un seul côté = écran vide ou contenu doublé.

### 2.7 Motion

| Interaction | Durée | Easing |
|---|---|---|
| survol, couleur de bordure | `--dur-1` | `--ease` |
| appui (contrôles ≤ 48 px, `scale(.96)`) | `--dur-1` | `--ease` |
| changement de semaine (glissement) | `--dur-3` | `--ease` |
| dialogue desktop (opacité + 8 px) / bottom-sheet | `--dur-3` | `--ease` |
| barre de progression | `--dur-4` | `--ease-inout` |
| toast entrée / sortie | `--dur-2` / `--dur-3` | `--ease` |
| spinner de synchro | 900 ms `linear infinite` | — |

**Conservés tels quels** (pilotés par `app.js`) : keyframes `mdSlideNext`, `mdSlidePrev`, `mdBumpNext`, `mdBumpPrev`, `sheetUp`, `dictaPulse`, `slideIn`, classes `.md-anim-next`, `.md-anim-prev`, `.md-bump-next`, `.md-bump-prev`.
**Interdits** : parallaxe, révélation au scroll, pulsation décorative, animation de `width`/`height` hors barres de progression.
`prefers-reduced-motion` est tué globalement dans `utilities.css` (chargé en dernier).

---

## 3. Contrat JS → CSS

**C'est la règle la plus importante du document.** Le CSS ne doit jamais dépendre d'un style inline arbitraire.

### 3.1 Règle

| Donnée dynamique | Attribut exact | Consommé par le CSS |
|---|---|---|
| Couleur d'établissement / d'absence | `style="--c:#rrggbb"` | `background: var(--c)`, `color: var(--c)`, `border-color: color-mix(in oklab, var(--c) 45%, transparent)` |
| Valeur d'une barre / progression | `style="--v:NN%"` | `width: var(--v)`, `height: var(--v)` |

- **Une seule** couleur dynamique par élément, sous le nom `--c`. Pas de `--ac`, pas de `--color`.
- **Une seule** valeur dimensionnelle dynamique, sous le nom `--v` (toujours un pourcentage).
- **Aucun autre attribut `style`** n'est autorisé dans le markup généré.
- Le CSS prévoit toujours un repli : `var(--c, var(--c-s3))`, `var(--v, 0%)`.
- Le CSS ne peint **jamais** de dégradé pour un bloc de shift (décision produit : aplat + rail).

### 3.2 Squelettes attendus (sortie JS)

```html
<!-- bloc de shift, grille desktop / vue établissement mobile -->
<button type="button" class="shift" data-id="sh_1" style="--c:#34d399" title="Note éventuelle">
  <span class="shift-time">06:00 – 13:30</span>
  <span class="shift-dur">7h30</span>
  <span class="shift-label">DRIVE</span>
</button>

<!-- bloc d'absence, grille -->
<button type="button" class="absence" data-absence="1" style="--c:#a78bfa" title="Congés">🌴 CONGÉS</button>

<!-- bloc d'absence, vue jour mobile -->
<button type="button" class="md-absence past" data-absence="1" style="--c:#a78bfa">🌴 CONGÉS</button>

<!-- progression de la barre de résumé -->
<div class="progress-wrap"><div class="progress" style="--v:81%"></div></div>

<!-- barre verticale (stats) -->
<div class="bar-col">
  <div class="bar-zone"><div class="bar cur" style="--v:64%"><span class="bar-tip">22h30</span></div></div>
  <span class="bar-x">S46</span>
</div>

<!-- barre horizontale (stats) -->
<div class="hbar-row">
  <div class="hbar-key"><span class="est-dot" style="--c:#34d399"></span>CHANTONNAY</div>
  <div class="hbar-track"><div class="hbar" style="--v:42%"></div></div>
  <div class="hbar-val">12h30</div>
</div>

<!-- pastille de couleur d'établissement (jamais background: inline) -->
<span class="est-dot" style="--c:#34d399"></span>

<!-- pastille de couleur sélectionnable (modale établissement) -->
<button type="button" class="color-dot sel" data-color="#34d399" style="--c:#34d399" aria-label="#34d399" aria-pressed="true"></button>
```

### 3.3 Points à modifier dans `app.js` (checklist durables)

Lignes de référence : `app.js` d'origine (1958 lignes).

| Ligne | Markup actuel | À produire |
|---|---|---|
| 262 | `style="background:${est.color}"` sur `.est-dot` | `style="--c:${est.color}"` |
| 287 | idem `.est-dot` | `style="--c:${est.color}"` |
| 296 | `.absence` : `border-color/color/background` + `${meta.color}55` / `14` | `class="absence" style="--c:${meta.color}"` (et `<button>`) |
| 299-303 | `.shift` : `background:linear-gradient(135deg, ${est.color}, ${shade(est.color,-0.3)})` | `class="shift" style="--c:${est.color}"` — **supprimer `shade()` du rendu** |
| 318-324 | `.progress` : `style="width:${pct}%"` | `style="--v:${pct}%"` |
| 375 | `.md-absence` : `border-color/color/background` | `style="--c:${meta.color}"` |
| 378-381 | `.md-shift` : `style="--c:${est.color}"` | **déjà conforme** |
| 416-420 | `.est-dot` dans `.md-est-head` | `style="--c:${est.color}"` |
| 424 | `.md-absence` (vue établissement) | `style="--c:${meta.color}"` |
| 427-430 | `.shift` (vue établissement mobile) : dégradé | `style="--c:${est.color}"` |
| 536-537 | `.color-dot` : `background:${c}` | `style="--c:${c}"` + `aria-label` + `aria-pressed` |
| 575 | `.absence-item` : `style="--ac:${meta.color}"` | `style="--c:${meta.color}"` |
| 631 | `.bar` : `style="height:${Math.max(3,pct)}%"` | `style="--v:${pct}%"` (le minimum 3 % passe dans le CSS : `min-height:3px`) |
| 646-649 | `.hbar` : `width:…%;background:${x.color}` | `style="--v:${pct}%;--c:${x.color}"` |
| 661-664 | `.hbar` : `width:…%;background:var(--blue)` | `style="--v:${pct}%"` (couleur par défaut dans le CSS) |

Ajouts d'attributs (aucune logique métier) :

| Ligne | Ajout |
|---|---|
| 180-186 | `toast()` : `t.setAttribute('role', type === 'error' ? 'alert' : 'status')` |
| 205 | `switchView()` : `b.setAttribute('aria-current', actif ? 'page' : 'false')`, et l'absence d'attribut sur les inactifs |
| 248-249 | `#mdWeekBtn` : **ne pas** mettre d'emoji dans le texte (`textContent` = « Semaine » / « Jour »), l'icône passe en CSS |
| 435 | `.md-empty` : rendre focusable (`<button type="button">`) |
| 694-695 | `.chip` : `aria-pressed="${état}"` |
| 863 | `renderSyncInfo()` : `el.dataset.state = t ? 'ok' : 'idle'` |
| 892-895 | `setSyncBtn()` : `b.textContent` sans glyphe + `b.toggleAttribute('aria-busy', mode === 'busy')` |
| 1023 / 1063 | `#icsSyncNowBtn` : `aria-busy="true"` pendant la synchro (déjà `disabled`) |
| 1137 | `#avatarBtn` : `aria-expanded` synchronisé avec `.hidden` du menu |
| 1298+ | `openShiftModal()` : `#shDelete` masqué ⇒ `disabled` aussi (les deux), jamais `.hidden` seul sur un bouton |

---

## 4. Catalogue de composants

Chaque composant liste : squelette **exact**, variantes, états, et le fichier CSS propriétaire.
Les attributs `id` sont contractuels (`app.js` les récupère) : **ne jamais les renommer**.

### 4.1 Bouton — `.btn`

```html
<button type="button" class="btn primary">＋ Ajouter un shift</button>
<button type="button" class="btn ghost">Aujourd'hui</button>
<button type="button" class="btn danger">Supprimer</button>
<button type="button" class="btn primary" aria-busy="true">Synchronisation…</button>
```

| Variante | Fond | Texte | Bord |
|---|---|---|---|
| `.primary` | `--c-accent` → `--c-accent-strong` (dégradé 135° autorisé **ici uniquement**) | `--c-on-accent` | transparent |
| `.ghost` | `--c-s2` | `--c-text` | `--c-border` |
| `.danger` | `--c-danger-soft` | `--c-danger` | `--c-danger` à 30 % |

États : `:hover` (brightness interdit → `--c-accent-strong` plein / `--c-s3` pour ghost), `:active` (`scale(.97)`), `:focus-visible` (anneau global), `:disabled` (`opacity:.45`, `cursor:not-allowed`, **aucune** ombre), `[aria-busy="true"]` (spinner `::before`, pointeur désactivé).
Hauteur mini `--hit` (44 px) ; jamais de `white-space` cassé (`nowrap`).
Propriétaire : `components.css`.

### 4.2 Bouton icône — `.icon-btn`

```html
<button type="button" class="icon-btn" id="prevWeek" aria-label="Semaine précédente" title="Semaine précédente (←)">
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
       stroke-width="2.5" stroke-linecap="round" aria-hidden="true" focusable="false">
    <path d="M15 18l-6-6 6-6"/>
  </svg>
</button>
```

**44 × 44 px obligatoire** (32 px aujourd'hui). `aria-label` obligatoire (le `title` seul ne suffit pas). SVG `aria-hidden` + `focusable="false"`.
Propriétaire : `components.css`.

### 4.3 Champ, libellé, aide, erreur — `.field`

```html
<label class="field">
  <span>Taux horaire (€/h)</span>
  <input type="text" inputmode="decimal" id="pRate" placeholder="12,50" aria-describedby="pRateHelp">
  <span class="field-help" id="pRateHelp">Utilisé pour estimer le salaire.</span>
</label>

<!-- avec erreur -->
<label class="field">
  <span>Relais privé (optionnel)</span>
  <input type="url" id="icsRelayUrl" aria-describedby="icsRelayHelp icsRelayError" aria-invalid="true">
  <span class="field-help" id="icsRelayHelp">URL de base seule, sans /ics.</span>
  <span class="field-error" id="icsRelayError" role="status">URL invalide…</span>
</label>
```

- Le libellé est `.field > span:first-child` (markup existant : `<span>` sans classe). `.field-label` est accepté comme alias pour le nouveau markup.
- `.field-help` : `--fs-2`, `--c-text-3`. `.field-error` : `--fs-2`, `--c-danger`, `role="status"`.
- Le bord du champ vient de `base.css` (`--c-border-field`, 3.6:1). L'erreur passe uniquement par `aria-invalid="true"` (posé par `app.js:879`), stylée par `:has()` ou `[aria-invalid]` — **jamais** par une classe `.error`.
- La hauteur du champ est `--hit` (44 px) sur desktop comme sur mobile ; `font-size:16px` sur mobile (anti-zoom iOS) via `mobile.css`.
Propriétaire : `components.css` (+ `base.css` pour le contrôle nu).

### 4.4 Chip — `.chip`

```html
<button type="button" class="chip on" aria-pressed="true">Lundi</button>
<button type="button" class="chip" aria-pressed="false">Mardi</button>
```

`padding: var(--sp-4) var(--sp-5)`, `border-radius: var(--r-pill)`, `min-height: var(--hit)` sur mobile.
État `on` : `--c-accent-soft` + `--c-accent-line` + `--c-accent` (7.17:1). État `off` : `--c-s2` + `--c-border` + `--c-text-2`.
**`aria-pressed` obligatoire** (l'état ne doit pas dépendre de la couleur seule).
Propriétaire : `components.css`.

### 4.5 Carte / panneau — `.panel`

```html
<section class="panel">
  <h3>Heures par semaine <span class="muted small">(8 dernières semaines)</span></h3>
  …
</section>

<section class="panel panel-inset"><!-- puits : fond --c-bg, filet interne --></section>
```

`background: var(--c-s1)`, `border: 1px solid var(--c-border)`, `border-radius: var(--r-lg)`, **aucune ombre**.
`.panel-inset` : `background: var(--c-bg)` — utilisé comme puits (grille, zone creusée dans une carte).
`.form-card` (profil) = même rendu, sémantique différente : conserve son nom.
Propriétaire : `components.css`.

### 4.6 Bloc de shift (grille) — `.shift`

```html
<button type="button" class="shift" data-id="sh_1" style="--c:#34d399" title="Note">
  <span class="shift-time">06:00 – 13:30</span>
  <span class="shift-dur">7h30</span>
  <span class="shift-label">DRIVE</span>
</button>
```

```css
.shift{
  --c: var(--c-accent);                 /* repli si JS oublie la variable */
  background: var(--c);                 /* APLAT — aucun dégradé */
  color: var(--c-shift-ink);
  border-left: 3px solid color-mix(in oklab, var(--c) 60%, #000);
  border-radius: var(--r-sm);
  box-shadow: none;
}
```

- `.shift` est un `<button>` (déjà le cas) : jamais un `<div>`.
- `.shift-dur` est positionné en haut à droite, `--fs-1`, `opacity:.85`.
- `.shift-label` : `--fs-1`, `600`, uppercase, `overflow:hidden; text-overflow:ellipsis`.
- États : `:hover` (`filter:brightness(1.06)` sur le bloc), `:active` (`brightness(.98)`), `:focus-visible` (anneau canvas + halo, cf. `utilities.css`), `.today` (n'existe pas sur `.shift`), `.past` (vue mobile seulement).
- Container query : sous 110 px de colonne, `.shift-label` disparaît (`@container (max-width:110px)`), le bloc ne garde que l'horaire.
Propriétaire : `planning.css`.

### 4.7 Ligne de shift (vue jour mobile) — `.md-shift`

```html
<button type="button" class="md-shift past" data-id="sh_1" style="--c:#60a5fa" title="Note">
  <span class="md-bar" aria-hidden="true"></span>
  <span class="md-row">
    <span class="md-time">17:00 – 21:00</span>
    <span class="md-estname">DRIVE</span>
    <span class="md-dur">4h</span>
  </span>
  <span class="md-label">CAISSE</span>
</button>
```

`.md-bar` = rail 5 px `background: var(--c)` (déjà conforme dans `app.js:378`).
Fond `--c-s1` + filet `--c-border`, `--r-md`, hover `--c-s2`, `.past{opacity:.55}`.
`.md-time` : `--fs-5`, 700, tabular. `.md-estname` : `--fs-2`, `--c-text-2`, ellipsis. `.md-dur` : `--fs-2`, `--c-text-2`.
Propriétaire : `mobile.css`.

### 4.8 Absence — `.absence` / `.md-absence`

```html
<button type="button" class="absence" data-absence="1" style="--c:#a78bfa" title="Congés">🌴 CONGÉS</button>
<button type="button" class="md-absence past" data-absence="1" style="--c:#a78bfa">🌴 CONGÉS</button>
```

```css
.absence{
  --c: var(--c-absence);
  border: 1px dashed color-mix(in oklab, var(--c) 45%, transparent);
  background: color-mix(in oklab, var(--c) 8%, transparent);
  color: var(--c);
  border-radius: var(--r-sm);
  font-size: var(--fs-1);
  font-weight: 600;
  text-transform: uppercase;
}
```

`<div data-absence>` devient `<button type="button">` : la délégation `closest('.absence')` (`app.js:1167`, `1184`) reste valide, et l'élément devient focusable.
Cliquer une absence ouvre la vue Absences (comportement existant, pas de modale).
Propriétaire : `planning.css` (grille) / `mobile.css` (`.md-absence`).

### 4.9 Résumé de semaine + estimation de salaire — `.summarybar`

```html
<footer class="summarybar" id="summaryBar">
  <div class="sum-item"><span class="sum-num">4</span><span>shifts</span></div>
  <div class="sum-item"><span class="sum-num">28h30</span><span>travaillées</span></div>
  <div class="progress-wrap"><div class="progress" style="--v:81%"></div></div>
  <div class="progress-label">Objectif 35 h · 81 %</div>
  <div class="sum-item"><span class="sum-num sum-money">356,25 €</span><span>estimés · 12,5 €/h</span></div>
</footer>
```

- `.sum-num` : `--fs-6`, 700, tabular. `.sum-money` : `--fs-8` (28) → `--fs-9` (36) à partir de 1024, `--c-accent`.
- `.progress-wrap` : piste `--c-s3`, hauteur 6 px, `--r-pill`. `.progress` : `width: var(--v, 0%)`, dégradé `--c-accent → --c-accent-strong` (le bleu est réservé à « aujourd'hui »).
- `.progress-label` : `--fs-2`, `--c-text-2`.
- Desktop ≥ 1024 : le bloc est **sticky en bas** (`position: sticky; bottom: 0`), fond `--c-s1` opaque + filet haut, pour garder le salaire visible.
Propriétaire : `planning.css` (structure) / `components.css` (barres).

### 4.10 Statistique — `.stat-card`

```html
<div class="cards">
  <div class="stat-card green">
    <div class="k">Heures cette semaine</div>
    <div class="v" id="statWeek">28h30</div>
    <div class="s" id="statWeekSub">objectif 35 h</div>
  </div>
  <div class="stat-card">
    <div class="k">Rémunération estimée (mois)</div>
    <div class="v" id="statPay">1 425 €</div>
    <div class="s" id="statPaySub">Selon votre taux horaire</div>
    <div class="mini" id="statPayMini"><div class="mini-bar" style="--v:81%"></div></div>
  </div>
</div>
```

`.k` : `--fs-2`, 600, `--c-text-2`. `.v` : `--fs-6`, 700, tabular. `.s` : `--fs-2`, `--c-text-3` (**plus** `--faint`).
`.green .v` → `--c-accent`, `.blue .v` → `--c-info`.
`.mini` : piste 4 px `--c-s3` ; `.mini-bar` : `width: var(--v, 0%)`.
Propriétaire : `screens.css`.

### 4.11 Progression et barres

```html
<!-- linéaire -->
<div class="progress-wrap"><div class="progress" style="--v:81%"></div></div>
<div class="mini"><div class="mini-bar" style="--v:60%"></div></div>

<!-- verticale (8 semaines) -->
<div class="bars">
  <div class="bar-col">
    <div class="bar-zone"><div class="bar cur" style="--v:64%"><span class="bar-tip">22h30</span></div></div>
    <span class="bar-x">S46</span>
  </div>
</div>

<!-- horizontale (répartitions) -->
<div class="hbar-row">
  <div class="hbar-key"><span class="est-dot" style="--c:#34d399"></span>CHANTONNAY</div>
  <div class="hbar-track"><div class="hbar" style="--v:42%"></div></div>
  <div class="hbar-val">12h30</div>
</div>
```

`.bar{ height: var(--v, 0%); min-height: 3px }` — `.bar.cur` = `--c-accent`, sinon `--c-info`.
`.hbar{ width: var(--v, 0%); background: var(--c, var(--c-info)) }`.
Toute transition de `width`/`height` est supprimée sous `prefers-reduced-motion` (déjà global).
Propriétaire : `screens.css`.

### 4.12 Pastille d'état — `.c-pill` (nouveau) + `.c-spinner`

```html
<span class="c-pill" id="icsLastSync" role="status" aria-live="polite" data-state="idle">Jamais synchronisé</span>
<span class="c-pill" data-state="busy"><span class="c-spinner" aria-hidden="true"></span>Synchronisation…</span>
<span class="c-pill" data-state="ok">Dernière synchro : 16/11 14:32</span>
<span class="c-pill" data-state="error">Relais injoignable</span>
```

`data-state` ∈ `idle` | `busy` | `ok` | `error` (posé par `app.js`, `renderSyncInfo`).

| `data-state` | Fond | Texte | Point |
|---|---|---|---|
| `idle` | `--c-s2` | `--c-text-3` | `--c-text-off` |
| `busy` | `--c-accent-soft` | `--c-accent` | `.c-spinner` |
| `ok` | `--c-accent-soft` | `--c-accent` | `--c-accent` |
| `error` | `--c-warn-soft` | `--c-warn` | `--c-warn` |

`.c-spinner` : 12 px, anneau 2 px `currentColor` avec un segment transparent, `animation: spin 900ms linear infinite` (nom libre, tué par `utilities.css` en reduced-motion → état stable statique).
Propriétaire : `components.css`.

### 4.13 Toast — `.toast`

```html
<div class="toast" role="status">Import Skello : 3 shift(s) ✓</div>
<div class="toast error" role="alert">Récupération impossible…</div>
```

Conteneur `#toasts` (positionné dans `layout.css`). `.toast` : `--c-s3`, filet gauche 3 px `--c-accent` (`.error` → `--c-danger`), `--sh-pop`, `--r-md`, `--fs-3`, 600.
Sortie : `.toast.out` (opacité + `translateY(6px)`), durée 300 ms, classe posée par `app.js:182`.
`role` **doit** être posé par `app.js` (voir §3.3).
Propriétaire : `components.css`.

### 4.14 Dialogue et bottom-sheet

```html
<dialog id="shiftModal" aria-labelledby="shTitle">
  <form id="shForm" autocomplete="off">
    <div class="dialog-head">
      <h3 id="shTitle">Nouveau shift</h3>
      <span class="dur-preview" id="shDur">7h30</span>
    </div>
    <div class="dialog-body">…</div>
    <div class="dialog-foot">
      <button type="button" class="btn danger" id="shDelete">Supprimer</button>
      <span class="spacer"></span>
      <button type="button" class="btn ghost" id="shCancel">Annuler</button>
      <button type="submit" class="btn primary">Enregistrer</button>
    </div>
  </form>
</dialog>
```

- `dialog` : `--c-s1`, `1px --c-border`, `--r-xl`, `--sh-pop`, `width: min(560px, 92vw)`, `max-height: 90dvh`, `overflow:auto`.
- `::backdrop` : `rgba(4,8,14,.72)`, **sans** `backdrop-filter`.
- `.dialog-head` : 56 px, sticky top, filet bas. `.dialog-foot` : sticky bottom, filet haut, fond `--c-s1`.
- Mobile (< 720) : `margin: auto auto 0`, `--r-xl --r-xl 0 0`, `--sh-sheet`, `animation: sheetUp var(--dur-3) var(--ease)` (nom de keyframe conservé), `.dialog-head::before` = grabber 36 × 4 px.
- `<h3 id>` obligatoire pour `aria-labelledby` : `shTitle`, `abTitle`, `cfTitle` existent ; **`estTitle` et `icsTitle` sont à créer**.
- Le `.dialog-body` peut recevoir `#dictaBtn` en dernier enfant (voir §9).

Propriétaire : `components.css` (desktop) / `mobile.css` (sheet).

### 4.15 Menu avatar — `.dropdown`

```html
<div class="avatar-wrap">
  <button type="button" class="avatar" id="avatarBtn" aria-label="Menu du compte"
          aria-haspopup="true" aria-expanded="false" title="Menu">AB</button>
  <div class="dropdown hidden" id="avatarMenu">
    <div class="dropdown-head">
      <div class="avatar" id="menuAvatar">AB</div>
      <div><div class="dd-name" id="menuName">Mon profil</div><div class="dd-mail" id="menuEmail">—</div></div>
    </div>
    <button type="button" data-action="goto-profile">Profil &amp; disponibilités</button>
    …
    <div class="c-sep" role="separator"></div>
    <button type="button" data-action="reset" class="danger">Réinitialiser…</button>
  </div>
</div>
```

- 280 px, `--c-s1`, `--c-border`, `--sh-pop`, `--r-lg`. Items `min-height: 44px`, hover `--c-s3`.
- Groupes séparés par `.c-sep` (filet 1 px, `--sp-2` de marge verticale) : navigation / données / danger.
- `.avatar` : 36 visuel, **zone de frappe 44** (`::after` inset -4px), `--c-info` → `#7c5bff`, texte `--c-on-info` (7.04:1 ; le blanc actuel est à 2.75:1).
- `aria-expanded` synchronisé par `app.js` (voir §3.3). Pas de `role="menu"` : sans navigation par flèches, ce serait moins conforme qu'un groupe de boutons.
Propriétaire : `layout.css` (positionnement) / `components.css` (rendu).

### 4.16 Barre de semaine — `.weekbar`

```html
<div class="weekbar">
  <div class="weekbar-left">
    <button type="button" class="icon-btn" id="prevWeek" aria-label="Semaine précédente">…</button>
    <div class="week-pill" id="weekLabel">2 – 8 déc.</div>
    <button type="button" class="icon-btn" id="nextWeek" aria-label="Semaine suivante">…</button>
    <span class="week-num" id="weekNum">Semaine 49</span>
    <button type="button" class="btn ghost" id="todayBtn">Aujourd'hui</button>
    <input type="date" id="jumpDate" class="date-jump" aria-label="Aller à la semaine contenant cette date">
    <button type="button" class="btn ghost md-week-toggle" id="mdWeekBtn" title="Afficher toute la semaine">Semaine</button>
  </div>
  <div class="weekbar-right">
    <button type="button" class="btn ghost hidden" id="syncIcsBtn" title="Récupérer le planning publié sur Skello">Sync Skello</button>
    <button type="button" class="btn primary" id="addShiftBtn">＋ Ajouter un shift</button>
  </div>
</div>
```

- `.week-pill` : `--c-s2`, filet, `--r-md`, `--fs-3`, 600, tabular, `white-space: nowrap`, `min-width: 148px`.
- `.week-num` : `--c-accent-soft`, filet `--c-accent-line`, texte `--c-accent`, `--fs-1`, 600, uppercase, `--r-pill`.
- `.date-jump` : 150 px, `--fs-2`, `--c-text-2` ; masqué < 720 px (existant).
- **Un seul CTA primaire par écran** : `#addShiftBtn`. `#syncIcsBtn` est un ghost.
- ⚠️ `#syncIcsBtn` et `#mdWeekBtn` voient leur `textContent` réécrit par `app.js` : **aucun élément enfant**, icône en `::before` CSS uniquement.
Propriétaire : `layout.css`.

### 4.17 État vide — `.empty-state`

```html
<div class="empty-state">
  <div class="empty-icon" aria-hidden="true">🌴</div>
  <h3>Planifiez vos absences</h3>
  <p>Demandez, visualisez et suivez vos absences.<br>Elles s'affichent directement dans le planning.</p>
  <button type="button" class="btn primary" id="addAbsenceBtn2">Demander une absence</button>
</div>
```

`.empty-state` : colonne centrée, `--sp-9` vertical, `gap: var(--sp-3)`.
`.empty-icon` : 56 px, `--r-pill`, `--c-accent-soft`, `--fs-8` (emoji décoratif ⇒ `aria-hidden`).
`h3` : `--fs-6`. `p` : `--fs-4` / `--fs-5` mobile, `--c-text-2`, `max-width: 46ch`.
CTA unique, optionnel.
Propriétaire : `screens.css`.

### 4.18 Liste d'absences — `.absence-item`

```html
<div class="absence-item" style="--c:#a78bfa">
  <div class="a-icon" aria-hidden="true">🌴</div>
  <div>
    <div class="a-type">Congés</div>
    <div class="a-dates">2 déc. → 6 déc. · <em>note</em></div>
  </div>
  <span class="badge-status">✓ Confirmée</span>
  <div class="a-actions">
    <button type="button" class="icon-btn" data-edit="ab_1" aria-label="Modifier">✎</button>
    <button type="button" class="icon-btn" data-del="ab_1" aria-label="Supprimer">🗑️</button>
  </div>
</div>
```

`background: --c-s1`, filet, `border-left: 4px solid var(--c, var(--c-absence))`.
`.badge-status` : `--c-accent-soft` + `--c-accent`, `--r-pill`, `--fs-1`, 600.
Les deux `.icon-btn` font 44 px et portent un `aria-label` (les glyphes ✎/🗑️ sont décoratifs).
Propriétaire : `screens.css`.

### 4.19 Carte établissement (rail et vue Établissements) — `.est-card`

```html
<div class="est-card" data-est="est_1" title="Modifier cet établissement">
  <div class="est-name"><span class="est-dot" style="--c:#34d399"></span><span>CHANTONNAY</span></div>
  <div class="est-hours">12h30 · 3 shifts cette semaine</div>
</div>
```

`.est-card` est un `div` cliquable (délégation `closest('.est-card')`) → **conserver `div`** : le rendre focusable est un ticket a11y séparé (voir §9, piège 12).
`.est-dot` : 10 px, `--r-sm` (3 px), `background: var(--c, var(--c-s3))`.
Propriétaire : `planning.css`.

---

## 5. Composition écran par écran

### 5.1 Planning — desktop ≥ 1024

```
┌ .topbar  sticky  h = --topbar-h ──────────────────────────────────────────┐
│ ◷ Créno      Planning  Absences  Stats  Profil        ● Sauvegardé  (AB)  │
└───────────────────────────────────────────────────────────────────────────┘
  .view · padding-inline --sp-8 · max-width --page-max
┌ .weekbar  h = 52 ─────────────────────────────────────────────────────────┐
│ ‹  [S46 · 10 – 16 nov]  ›   [Aujourd'hui]  [10 nov.]      ⟳ Sync  ＋ Ajouter│
└───────────────────────────────────────────────────────────────────────────┘
┌ .est-panel  --rail-w ┬ .grid-wrap  (--c-s1, --r-lg, 1 filet) ─────────────┐
│ TOTAUX               │        LUN 10   MAR 11   MER 12   …                │ ← head sticky 40
│ ● CHANTONNAY         │ 12h30  ┌──────┐                                    │
│   12h30 · 3 shifts   │        │06:00 │   bloc plein var(--c), rail 3px    │
│ ● DRIVE              │        │13:30 │   encre --c-shift-ink              │
│   6h · 2 shifts      │        └──────┘                                    │
│ ──────────────────── │  filets 1px --c-border-soft entre cases             │
│ [+ établissement]    │                                                    │
└──────────────────────┴────────────────────────────────────────────────────┘
┌ .summarybar  sticky bottom  h = 72 ───────────────────────────────────────┐
│ 4 shifts · 28h30 · ▓▓▓▓▓▓▓░░ 81 % (obj. 35 h)        356,25 €  estimés    │
└───────────────────────────────────────────────────────────────────────────┘
```

- Le rail **n'est plus un doublon** de la 1ʳᵉ colonne : il porte les **totaux** (heures, nb de shifts) + `#estAddForm` en pied.
- Grille : `background: var(--c-bg)` (puits), cellules transparentes, **un seul cadre** au lieu de N cartes.
- `.week-grid` : `grid-template-columns: var(--rail-w) repeat(7, minmax(100px, 1fr))` — **plus de `min-width: 1080px`**.
- `.cell` : `min-height: 96px`, `padding: var(--sp-2)`, gap `var(--sp-1)`.
- Tête de jour : `LUN` 11/600 `--c-text-3`, numéro 20/700 tabular ; aujourd'hui = pastille `--c-info` + texte `--c-on-info`, **sans glow**.
- `.grid-wrap` : `isolation: isolate`, `container-type: inline-size`.
- Empilement sticky conservé : corner 6 / head 5 / sticky-left 4.

### 5.2 Planning — tablette 720 → 1023

```
┌ .topbar (nav horizontale, pas de nav basse) ─────────────┐
├ .weekbar : 2 lignes (navigation / actions) ──────────────┤
├ .est-panel → bandeau horizontal scrollable ──────────────┤
│ (● CHANTONNAY 12h30) (● DRIVE 6h) (+)                    │
├ .grid-wrap  scroll horizontal + colonne sticky ──────────┤
└ .summarybar (non sticky) ────────────────────────────────┘
```
La grille peut défiler horizontalement, la première colonne reste collée. On ne descend **pas** à la vue mobile avant 720.

### 5.3 Planning — mobile < 720

```
┌ .topbar h=48 : ◷ Créno                                    (AB) ┐
├ ‹ [S46 · 2 – 8 déc.] ›          [Aujourd'hui] [Semaine]        ┤ 2 lignes
├ .md-date ─────────────────────────────────────────────────────┤
│ Lundi 2 déc. · aujourd'hui                      ⟳ Sync         │
│ ┌▌ CHANTONNAY ───────────────────────── 7h30 ──┐              │
│ │ 06:00 – 13:30                       DRIVE     │              │
│ └───────────────────────────────────────────────┘              │
│ ┌▌ DRIVE ─────────────────────────────── 4h ────┐              │
│ │ 17:00 – 21:00                       CAISSE     │              │
│ └───────────────────────────────────────────────┘              │
│ [               ＋ Ajouter un shift              ]  44px .primary
├ .summarybar ───────────────────────────────────────────────────┤
│ 2 shifts · 10h30                              131,25 €         │
│ ▓▓▓▓▓▓░░░░░░░ 30 % de l'objectif 35 h                          │
└────────────────────────────────────────────────────────────────┘
[ Planning  Absences  Stats  Profil ]   nav fixe --nav-h + safe-area
```

- `body` : `padding-bottom: calc(var(--nav-h) + env(safe-area-inset-bottom))`.
- Navigation des jours **inchangée** : swipe horizontal + bascule `#mdWeekBtn` (Jour/Semaine). Aucune barre de jours (ce serait une fonctionnalité nouvelle).
- `.md-shift` : hauteur mini 56 px, rail 5 px, `--fs-5` pour l'horaire.
- `#mdAdd` : pleine largeur, `--hit` minimum.

### 5.4 Sélecteur de semaine

| Élément | Spécification |
|---|---|
| Flèches | `.icon-btn` 44 × 44, `--c-s2`, hover `--c-s3`, actif `scale(.96)` |
| Pastille | `.week-pill` `--fs-3` 600 tabular, min 148 px, jamais de retour à la ligne |
| Numéro ISO | `.week-num` chip accent soft, 11/600 uppercase |
| Aujourd'hui | `.btn.ghost`, revient à la semaine courante **et** replace la vue jour sur aujourd'hui |
| Date | `#jumpDate` 150 px, ≥ 720 px uniquement |
| Bascule mobile | `#mdWeekBtn` : texte seul (JS), icône en `::before` |

Micro-interactions : changement de semaine = glissement `--dur-3` en mobile, **aucune animation** en desktop (les libellés se mettent à jour, c'est tout).

### 5.5 Carte d'estimation de salaire

Deux emplacements, un seul langage :

```
.résumé de semaine
  4 shifts    28h30    ▓▓▓▓▓▓▓▓░░░ 81 % · obj. 35 h
  --fs-2      --fs-6   6px, accent → accent-strong

  356,25 €  estimés                          12,50 €/h
  --fs-9 / --fs-8 · 700 · tabular · accent    --fs-2 · --c-text-3
```

Règles : **un seul chiffre en accent par écran** (le montant) ; le « € » est à 0.6 em ; pas de `0 €` quand le taux est vide (`—` géré par JS) ; jamais de pourcentage inventé.

### 5.6 Panneau de synchronisation ICS / Skello

```
┌ 📅 Importer depuis Skello                              ✕ 44 ┐ head 56 sticky
├ .dialog-body (scroll) ───────────────────────────────────────┤
│ ┌ .sync-meta  --c-s2, --r-md ─────────────────────────────┐  │
│ │ ● Jamais synchronisé     [⟳ Synchroniser maintenant]    │  │
│ │   .c-pill[data-state]     .btn.ghost                    │  │
│ └─────────────────────────────────────────────────────────┘  │
│ Comment faire                          ← .field-label 11 uppercase
│ Mon profil → Synchroniser le calendrier → copier l'URL        │
│ ┌ URL Skello ─────────────────────────── [Enregistrer] ──┐   │
│ └─────────────────────────────────────────────────────────┘   │
│ ▸ Relais privé (optionnel)      ← bloc secondaire, 1 filet    │
│   champ + aide 12px + erreur inline (aria-invalid)            │
│ ▸ Relais publics (case + avertissement)                       │
│ ─────────────────── ou manuellement ───────────────────       │
│ [📄 Choisir un fichier .ics]   ┌ coller le contenu ┐          │
│                                └───────────────────┘          │
│                                   [Analyser et importer]      │
└───────────────────────────────────────────────────────────────┘
```

| État | Déclencheur | Rendu |
|---|---|---|
| Jamais synchronisé | `lastIcsSync = 0` | `.c-pill[data-state=idle]` |
| En cours | `#icsSyncNowBtn[disabled]` + `aria-busy` | `.c-pill[data-state=busy]` + `.c-spinner` + barre indéterminée 2 px en haut du dialogue |
| Succès daté | `toLocaleString('fr-FR')` | `.c-pill[data-state=ok]` |
| Erreur relais | `#icsRelayError` non vide | `.field-error` + `aria-invalid="true"` + `.c-pill[data-state=error]` |
| Échec global | toast `RELAY_UNREACHABLE` | `.toast.error` `role="alert"` |
| Relais privé actif | `relayUrl` non vide | badge « Relais privé » + phrase « seuls direct et ce relais » |
| Extension (direct) | `IS_EXTENSION` | `.help-line` info : `--c-info-soft` + filet `--c-info` |
| Relais publics | `#icsAllowProxy` | case + texte d'avertissement `--c-text-3` |

Le parcours manuel **reste visible** (pas de `<details>`).

### 5.7 Absences

```
┌ .page-head ─────────────────────────────────────────────┐
│ Absences                              [＋ Demander une absence]  ← seul primaire
└─────────────────────────────────────────────────────────┘
┌ .absence-item  border-left 4px var(--c) ────────────────┐
│ [🌴]  Congés                        ✓ Confirmée  [✎][🗑] │
│       2 déc. → 6 déc. · note                             │
└─────────────────────────────────────────────────────────┘
```
Vide : `.empty-state` (§4.17), CTA dans l'état vide **et** dans `.page-head` (deux entrées, un seul style).

### 5.8 Statistiques

```
.page-head : h2 « Statistiques » + .muted.small (calculs 100 % locaux)
.cards      : 4 × .stat-card (auto-fit minmax(210px,1fr))
.panel      : Heures par semaine (8 dernières) → .bars
.panel      : Répartition par établissement → .hbar-row
.panel      : Top des postes → .hbar-row
```
Sur 375 px : cartes en 1 colonne, `.bars` avec `--fs-1` pour `.bar-tip`, `.hbar-row` en `96px 1fr 52px`.

### 5.9 Profil

```
≥ 1024 : [carte identité 320] [formulaire 1fr]
<  1024 : [carte identité] au-dessus du formulaire
```
Identité : `.avatar-xl` 84 px `--r-md`, nom 18/700, sous-titre `--c-text-2`.
Formulaire : `.form-grid` 2 colonnes ≥ 720, 1 colonne en dessous, `.field` espacés de `--sp-5`.
Actions : `Enregistrer` (primaire) puis, séparé par `--sp-8` et un filet, `Réinitialiser les données` (danger).
Disponibilités : `.chips` avec `aria-pressed`.

### 5.10 Établissements

`.form-card` : formulaire d'ajout (`.row`) + liste de `.est-card` (max 40 caractères, uppercase à l'enregistrement).
La modale couleur affiche `.color-dot` (44 × 44 zone, 24 visuel) avec `aria-pressed`.

### 5.11 Les cinq dialogues

| `id` | `aria-labelledby` | Contenu | Pied |
|---|---|---|---|
| `#shiftModal` | `shTitle` | établissement, date, poste, début, fin, note, avertissement disponibilité | Supprimer (masqué si création) / Annuler / Enregistrer |
| `#absenceModal` | `abTitle` | type, du, au, note | idem |
| `#estModal` | **`estTitle` (à créer)** | nom, 8 pastilles de couleur | Supprimer / Annuler / Enregistrer |
| `#icsModal` | **`icsTitle` (à créer)** | état de synchro, URL, relais privé, relais publics, parcours manuel | pas de pied (fermeture par ✕) |
| `#confirmModal` | `cfTitle` | message (`#cfMsg`) | Annuler / Confirmer (danger ou primaire selon le contexte) |

Mobile : bottom-sheet. Le pied reste **sticky** ; les boutons prennent `flex: 1` et `.spacer` disparaît (existant `styles.css:530-531`).
`#shWarn` / `.warn-line` : `--c-warn-soft` + filet `--c-warn` + texte `--c-warn`, `role="status"`.

### 5.12 États vides (tableau)

| Où | Source | Rendu |
|---|---|---|
| Grille sans établissement | `app.js:281` `.grid-empty` | puits `--c-bg`, icône 48 px `--c-accent-soft`, texte `--fs-5` `--c-text-2` — **pas de bouton** (le template n'en contient pas) |
| Rail sans établissement | `app.js:254` `.est-empty` | `--fs-2`, `--c-text-3`, texte court |
| Jour vide (mobile) | `app.js:435` `.md-empty` | carte pointillée, focusable (ticket `app.js`) |
| Journée sans shift (mode semaine) | `app.js:383` `.md-nochip` | chip `--c-s2`, `--fs-2` |
| Absences vides | `index.html:105` | `.empty-state` complet |
| Stats sans données | `app.js:651,666` | `.muted` 14 px + aide 12 px |
| Profil vide | `#profileAvatar` = `?` | avatar neutre, hint `--c-text-2` |

---

## 6. Vocabulaire de classes

**Règles**

1. On **conserve** tout nom utilisé par la délégation d'événements de `app.js` (renommer = panne silencieuse).
2. On conserve les classes **porteuses de sens** (structuration du contenu).
3. Les **nouveaux** composants sont préfixés `c-` (`.c-pill`, `.c-spinner`, `.c-sep`).
4. Un nom = un seul usage. Toute variante est un **modificateur** de ce nom (`.panel-inset`, `.md-shift.past`), jamais un nouveau composant.
5. Aucun `id` renommé. Aucun style inline hors `--c` / `--v`.

| Classe | Statut | Rôle / CSS propriétaire |
|---|---|---|
| `#mainNav`, `.nav-link`, `.active` | conservé | nav horizontale — `layout.css` |
| `#avatarBtn`, `.avatar`, `.avatar-wrap`, `.avatar-xl` | conservé | `layout.css` / `components.css` |
| `.dropdown`, `.dropdown-head`, `.dd-name`, `.dd-mail`, `.danger` | conservé | menu — `layout.css` |
| `.c-sep` | **nouveau** | séparateur de menu — `components.css` |
| `.save-state`, `.flash` | conservé | indicateur de sauvegarde — `layout.css` |
| `.view` | conservé | conteneur d'écran (`padding` + `max-width`) — `layout.css` |
| `.page-head` | conservé | titre + action d'écran — `layout.css` |
| `.btn`, `.primary`, `.ghost`, `.danger` | conservé | `components.css` |
| `.icon-btn` | conservé | `components.css` (44 px) |
| `.field`, `.field-help`, `.field-error`, `.form-grid`, `.field-block`, `.field-label` | `.field*` conservé, `.field-help`/`.field-error` **nouveaux** | `components.css` |
| `.chip`, `.chips`, `.on` | conservé | `components.css` |
| `.panel`, `.panel-inset` | `.panel` conservé, `-inset` **nouveau** | `components.css` |
| `.form-card` | conservé | profil — `screens.css` |
| `.weekbar`, `.weekbar-left`, `.weekbar-right`, `.week-pill`, `.week-num`, `.date-jump`, `.md-week-toggle` | conservé | `layout.css` |
| `.planning-layout`, `.est-panel`, `.est-panel-title`, `.est-list`, `.grid-wrap` | conservé | `layout.css` |
| `.week-grid`, `.grid-head`, `.corner`, `.day-head`, `.sticky-left`, `.dh-name`, `.dh-num`, `.dh-month`, `.badge` | conservé | `planning.css` |
| `.cell`, `.today`, `.est-cell`, `.grid-empty` | conservé | `planning.css` |
| `.shift`, `.shift-time`, `.shift-dur`, `.shift-label` | conservé | `planning.css` |
| `.absence`, `.md-absence` | conservé (devient `<button>`) | `planning.css` / `mobile.css` |
| `.summarybar`, `.sum-item`, `.sum-num`, `.sum-money`, `.progress-wrap`, `.progress`, `.progress-label` | conservé | `planning.css` |
| `.mobile-day`, `.md-*` (`md-date`, `md-today`, `md-sync`, `md-est`, `md-est-head`, `md-est-total`, `md-days`, `.md-dayblock`, `md-dayhead`, `md-hline`, `md-dhead`, `md-dnum`, `md-cards`, `md-shift`, `md-bar`, `md-row`, `md-time`, `md-estname`, `md-dur`, `md-label`, `md-nochip`, `md-empty`, `md-add`, `past`) | conservé | `mobile.css` |
| `.md-anim-next`, `.md-anim-prev`, `.md-bump-next`, `.md-bump-prev` | conservé (pilotées par JS) | `mobile.css` |
| `.cards`, `.stat-card`, `.k`, `.v`, `.s`, `.green`, `.blue`, `.mini`, `.mini-bar` | conservé | `screens.css` |
| `.bars`, `.bar-col`, `.bar-zone`, `.bar`, `.bar-tip`, `.bar-x`, `.cur` | conservé (`.bar-col` → garder, ne pas renommer) | `screens.css` |
| `.hbar-row`, `.hbar-key`, `.hbar-track`, `.hbar`, `.hbar-val` | conservé | `screens.css` |
| `.profile-layout`, `.profile-head`, `.profile-name`, `.avatar-xl` | conservé | `screens.css` |
| `.absence-list`, `.absence-item`, `.a-icon`, `.a-type`, `.a-dates`, `.a-actions`, `.badge-status` | conservé | `screens.css` |
| `.empty-state`, `.empty-icon` | conservé | `screens.css` |
| `.est-card`, `.est-name`, `.est-dot`, `.est-hours`, `.est-empty`, `.est-add` | conservé | `planning.css` |
| `.color-row`, `.color-dot`, `.sel` | conservé (+ `aria-pressed`) | `components.css` |
| `.row`, `.spacer`, `.sep-line`, `.help-line`, `.warn-line`, `.check`, `.sync-meta`, `.dur-preview` | conservé | `components.css` |
| `.toast`, `.error`, `.out`, `#toasts` | conservé (+ `role`) | `components.css` |
| `.c-pill`, `.c-spinner` | **nouveaux** | `components.css` |
| `dialog`, `.dialog-head`, `.dialog-body`, `.dialog-foot` | conservé | `components.css` / `mobile.css` |
| `.drop-overlay`, `.box` | conservé | `layout.css` |
| `#dictaBtn`, `.is-listening`, `#dictaPill`, `.is-error`, `.dicta-pill-dot`, `#dictaLive` | conservé (**contrat figé**) | `dicta.css` |
| `.sr-only`, `.hidden`, `.muted`, `.small`, `.num`, `.isolate`, `.cq` | conservé / **nouveaux helpers** | `utilities.css` |

**Suite réservée (ne pas réutiliser)** : `c-*` appartient aux nouveaux composants ; `.is-*` est réservé aux états posés par JS.

---

## 7. Mapping des tokens legacy

Les anciens noms **n'existent plus**. Toute occurrence dans un nouveau CSS est un bug.

| Ancien | Nouveau |
|---|---|
| `--bg` | `--c-bg` |
| `--panel` | `--c-s1` |
| `--panel-2` | `--c-s2` |
| `--panel-3` | `--c-s3` |
| `--border` | `--c-border` |
| `--border-soft` | `--c-border-soft` |
| `--text` | `--c-text` |
| `--muted` | `--c-text-2` |
| `--faint` | `--c-text-3` (informatif) / `--c-text-off` (décoratif) |
| `--accent` | `--c-accent` |
| `--accent-2` | `--c-accent-strong` |
| `--blue` | `--c-info` |
| `--danger` | `--c-danger` |
| `--radius` | `--r-lg` |
| `--shadow` | `--sh-pop` |
| `--ac` (inline JS) | `--c` |
| `--c` (déjà utilisé sur `.md-shift`) | `--c` (inchangé) |

---

## 8. Accessibilité

### 8.1 Contrastes — non négociables

Voir le tableau §1.2. Règles d'usage :

- Texte < 18.66 px gras / < 24 px : **≥ 4.5:1** ⇒ `--c-text-3` minimum. `--c-text-off` **interdit** pour du texte lisible.
- Éléments d'interface (bord de champ, anneau, icône porteuse de sens) : **≥ 3:1**.
- L'information n'est **jamais** portée par la couleur seule : « aujourd'hui » a son texte, les chips ont `aria-pressed`, les pastilles ont un libellé, les shifts ont une heure et un libellé.

### 8.2 Cibles tactiles

Minimum **44 × 44 px** (`--hit`) pour : `.btn`, `.icon-btn`, `.chip`, `.color-dot`, `.avatar`, `.md-sync`, `.md-nochip`, `.md-add`, `.dropdown button`, `.navbar` items, `#dictaBtn`.
Un visuel plus petit est autorisé si la zone de frappe est étendue (`::after { position:absolute; inset:-6px }`) — documenter au cas par cas.

### 8.3 Clavier

| Chemin | Comportement attendu |
|---|---|
| `Tab` | topbar → semaine → rail → grille → résumé ; aucun piège de focus |
| `←` / `→` | semaine précédente / suivante (hors champ et hors dialogue) — existant |
| `n` | nouveau shift sur aujourd'hui — existant |
| `Ctrl/Cmd + Maj + M` | dictée — existant |
| `Échap` | ferme dialogue et menu |
| `Entrée` / `Espace` | active bouton, chip, bloc de shift, absence |

Le focus ne doit jamais être masqué par `.topbar` ni par la nav basse (`scroll-padding` fourni par `utilities.css`).

### 8.4 ARIA — checklist

| Élément | Attribut | État |
|---|---|---|
| `.nav-link.active` | `aria-current="page"` | à ajouter (`app.js:205`) |
| `#avatarBtn` | `aria-haspopup="true"` + `aria-expanded` | à ajouter |
| `.chip` | `aria-pressed` | à ajouter (`app.js:694`) |
| `.toast` | `role="status"` / `role="alert"` | à ajouter (`app.js:180`) |
| `#icsLastSync` | `role="status" aria-live="polite"` + `data-state` | à ajouter |
| `#icsSyncNowBtn` | `aria-busy` pendant la synchro | à ajouter |
| `dialog` | `aria-labelledby` → `h3[id]` | `estTitle` / `icsTitle` à créer |
| `.icon-btn`, `.color-dot` | `aria-label` | obligatoire |
| SVG décoratif | `aria-hidden="true"` + `focusable="false"` | obligatoire |
| `#dictaLive` | `role="status" aria-live="polite"` | existant, ne pas toucher |
| `#icsRelayError` | `role="status"` | existant |

Pas de `role="menu"` sur `.dropdown` (aucune navigation par flèches implémentée : un rôle sans clavier conforme est pire que pas de rôle).

---

## 9. Pièges d'implémentation

**Contrats JS**

1. **Dictaphone re-parenté** : `app.js` déplace `#dictaBtn` et `#dictaPill` en derniers enfants de `.dialog-body` quand un dialogue est ouvert. Les règles `:has()` de `dicta.css` en dépendent. Ne pas changer `.dialog-body` en `grid` sans `order`, ni lui donner `overflow:hidden` (`position:sticky` du bouton cassé). `dialog:has(.dialog-body > #dictaBtn){ scroll-padding-bottom:76px }` doit rester.
2. **`:has()` est obligatoire** pour les états dérivés : `.dialog-body:has(> #dictaBtn)`, `.field:has(input[aria-invalid="true"])`, `.sync-meta:has(#icsSyncNowBtn[aria-busy="true"])`. Ne pas remplacer par une classe posée en JS si l'attribut existe déjà.
3. **Seuil 720** : deux arbres DOM rendus en permanence (`#planning-layout` masqué, `#mobileDayView` affiché). Le basculement est **100 % CSS**. Un seul côté modifié = écran vide ou doublons.
4. **`.hidden` = `display:none !important`** : `dictaIsUsable` (`app.js:1617`) teste `getComputedStyle(n).display === 'none'`. Masquer avec `visibility`, `opacity` ou `content-visibility` rend l'élément focusable et dicter-able.
5. **`content: attr(data-emoji)` supprimé** : les émojis de nav sont remplacés par des SVG inline `aria-hidden` (les pseudo-éléments sont lus par certains lecteurs d'écran et dupliquent le libellé).
6. **Textes réécrits par JS** : `#syncIcsBtn` (`app.js:892`), `#mdWeekBtn` (`app.js:248`), `#weekLabel`, `#weekNum`, `#icsLastSync`, `#saveState`. Aucun enfant, aucune icône dans ces nœuds.
7. **Styles inline** : seuls `--c` et `--v` sont autorisés. Un `style="background:…"` ou `style="width:…"` restant dans `app.js` écrase le CSS : voir la checklist §3.3.
8. **Classes de délégation** : `.shift .cell .est-cell .est-card .md-* .chip .color-dot .nav-link .view .dialog-body .toast .btn .field .absence [data-*]` — renommage = événements morts.
9. **`md-anim-*` / `md-bump-*`** : ajoutées après un reflow forcé (`void wrap.offsetWidth`). Les keyframes et le bloc `prefers-reduced-motion` doivent exister, sinon l'animation ne rejoue pas.
10. **`#toasts` et `#dictaBtn`** : offsets calculés à partir de la nav basse (`right: 74–84px`, `bottom: 72–132px`). Toute nouvelle valeur de `--nav-h` doit être reportée dans ces règles (`layout.css`, `dicta.css`).

**Mise en page**

11. **`calc(100vh - 235px)` interdit** : utiliser `100dvh` + `--topbar-h` / `--nav-h` / `env(safe-area-inset-bottom)`.
12. **`.week-grid{min-width:1080px}` supprimé** : `var(--rail-w) repeat(7, minmax(100px, 1fr))`. Le défilement horizontal ne subsiste qu'entre 720 et 1023 px.
13. **Empilement sticky** : corner 6 / head 5 / sticky-left 4, `isolation: isolate` sur `.grid-wrap`.
14. **Pas de `backdrop-filter`** : décision performance existante (blur plein écran interdit, et `backdrop-filter` sur un ancêtre crée un containing block qui piégerait la nav `fixed` et le sheet).
15. **`.topbar` reste `position: sticky`** ; en mobile elle est `static`/`relative` (la nav est en bas).
16. **Safe areas** : `viewport-fit=cover` déjà présent ; toujours `calc(var(--nav-h) + env(safe-area-inset-bottom))`, et `padding-inline` avec `env(safe-area-inset-left/right)` en paysage.
17. **`.cell` / `.est-cell` sont des `div` cliquables** : ne pas les convertir sans le ticket `app.js` correspondant, mais ne pas aggraver (ajouter un `<button>` dans une cellule est interdit : la délégation deviendrait ambiguë).

**Déploiement**

18. **`sw.js` sert le CSS en cache-first** : incrémenter `CACHE` (`sw.js:26`) et ajouter les nouveaux chemins à `ASSETS`, sinon la refonte n'arrive jamais chez l'utilisateur existant.
19. **`meta theme-color` et `manifest`** restent `#0a0e15` (barre système alignée sur le canvas).
20. **`tests/e2e/relay-sync.mjs`** sélectionne des éléments par texte : lancer `bash tools/run-tests.sh` après la refonte.

---

## 10. Ordre de chargement des CSS

**Ordre figé.** Toute inversion casse la cascade (les utilitaires doivent être derniers).

```
1. styles/tokens.css        :root + html (color-scheme)
2. styles/base.css          reset, canvas, typo, contrôles natifs, scrollbars
3. styles/layout.css        topbar, nav, .view, .weekbar, rail, grid-wrap, toasts, drop-overlay
4. styles/components.css    .btn, .field, .chip, .panel, .toast, dialogs, .c-pill, .c-sep
5. styles/planning.css      .week-grid, .cell, .shift, .absence, .summarybar, .est-card
6. styles/mobile.css        < 720 : nav basse, .md-*, bottom-sheet, vue jour
7. styles/screens.css       absences, stats, profil, établissements, .empty-state
8. styles/dicta.css         #dictaBtn, #dictaPill, re-parentage :has()
9. styles/utilities.css     .hidden, .sr-only, focus global, reduced-motion, scroll-padding
```

Dans `index.html`, dans cet ordre exact :

```html
<link rel="stylesheet" href="styles/tokens.css">
<link rel="stylesheet" href="styles/base.css">
<link rel="stylesheet" href="styles/layout.css">
<link rel="stylesheet" href="styles/components.css">
<link rel="stylesheet" href="styles/planning.css">
<link rel="stylesheet" href="styles/mobile.css">
<link rel="stylesheet" href="styles/screens.css">
<link rel="stylesheet" href="styles/dicta.css">
<link rel="stylesheet" href="styles/utilities.css">
```

`styles.css` (monolithe) est **supprimé** du chargement une fois les 9 fichiers en place, et retiré de `sw.js` `ASSETS`.

---

## 11. Checklist de recette

- [ ] Aucun hex dans un composant : uniquement `var(--…)`.
- [ ] Aucun `outline: none`, aucun `:focus` sans `:focus-visible`.
- [ ] Aucun `font-weight: 800`.
- [ ] Aucun `style="background|width|height"` dans `app.js` : seulement `--c` / `--v`.
- [ ] Aucune classe renommée parmi celles listées §6.
- [ ] Captures 375 / 768 / 1024 / 1440 px, aucun débordement horizontal.
- [ ] Clavier : `Tab` complet sur planning, absences, stats, profil ; focus toujours visible.
- [ ] `prefers-reduced-motion` : aucune animation restante (spinner figé, glissement instantané).
- [ ] États de synchro vérifiés : jamais / en cours / succès / erreur / relais privé / extension.
- [ ] Contrastes AA vérifiés sur les 4 largeurs (DevTools) et sur un bloc de shift par couleur de `PALETTE`.
- [ ] `bash tools/run-tests.sh` vert.
- [ ] `CACHE` de `sw.js` incrémenté.
