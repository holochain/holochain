const noop = () => null
const innerHTML = (x, y) => x.innerHTML = y
const CREATE_EVENT = 'create'

const observableEvents = [CREATE_EVENT]

const reactiveFunctions = {}

function react(link) {
  (reactiveFunctions[link] || noop)()
}

const store = createStore({}, react)

function update(target, compositor) {
  const html = compositor(target)
  if(html) innerHTML(target, html)
}

function draw(link, compositor) {
  listen(CREATE_EVENT, link, (event) => {
    const draw = update.bind(null, event.target, compositor)
    reactiveFunctions[link] = draw
    draw()
  })
}

function flair(link, stylesheet) {
  const styles = `
    <style type="text/css" data-module=${link}>
      ${stylesheet.replaceAll('&', link)}
    </style>
  `;

  document.body.insertAdjacentHTML("beforeend", styles)
}

export function learn(link) {
  return store.get(link) || {}
}

export function teach(link, knowledge, nuance = (s, p) => ({...s,...p})) {
  store.set(link, knowledge, nuance)
}

export function when(link1, eventName, link2, callback) {
  listen(eventName, `${link1} ${link2}`, callback)
}

export default function module(link, initialState = {}) {
  teach(link, initialState)

  return {
    link,
    learn: learn.bind(null, link),
    draw: draw.bind(null, link),
    flair: flair.bind(null, link),
    when: when.bind(null, link),
    teach: teach.bind(null, link),
  }
}

export function listen(type, link, handler = () => null) {
  const callback = (event) => {
    if(event.target && event.target.matches && event.target.matches(link)) {
      handler.call(null, event);
    }
  };

  document.addEventListener(type, callback, true);

  if(observableEvents.includes(type)) {
    observe(link);
  }

  return function unlisten() {
    if(type === CREATE_EVENT) {
      disregard(link);
    }

    document.removeEventListener(type, callback, true);
  }
}

let links = []

function observe(link) {
  links = [...new Set([...links, link])];
  maybeCreateReactive([...document.querySelectorAll(link)])
}

function disregard(link) {
  const index = links.indexOf(link);
  if(index >= 0) {
    links = [
      ...links.slice(0, index),
      ...links.slice(index + 1)
    ];
  }
}

function maybeCreateReactive(targets) {
  targets
    .filter(x => !x.reactive)
    .forEach(dispatchCreate)
}

function getSubscribers({ target }) {
  if(links.length > 0)
    return [...target.querySelectorAll(links.join(', '))];
  else
    return []
}

function dispatchCreate(target) {
  if(!target.id) target.id = sufficientlyUniqueId()
  target.dispatchEvent(new Event(CREATE_EVENT))
  target.reactive = true
}

new MutationObserver((mutationsList) => {
  const targets = [...mutationsList]
    .map(getSubscribers)
    .flatMap(x => x)
  maybeCreateReactive(targets)
}).observe(document.body, { childList: true, subtree: true });

function sufficientlyUniqueId() {
  // https://stackoverflow.com/a/2117523
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
    const r = Math.random() * 16 | 0, v = c == 'x' ? r : (r & 0x3 | 0x8);
    return v.toString(16);
  });
}

function createStore(initialState = {}, subscribe = () => null) {
  let state = {
    ...initialState
  };

  return {
    set: function(link, knowledge, nuance) {
      const wisdom = nuance(state[link] || {}, knowledge);

      state = {
        ...state,
        [link]: wisdom
      };

      subscribe(link);
    },

    get: function(link) {
      return state[link];
    }
  }
}
