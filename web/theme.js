(() => {
  const key = "loop-theme";
  const valid = (value) => ["system", "light", "dark"].includes(value);
  let preference = "system";
  try {
    const saved = localStorage.getItem(key);
    if (valid(saved)) preference = saved;
  } catch (error) {
    if (error.name !== "SecurityError") throw error;
    console.warn("Loop cannot read your saved theme; using the system theme.", error);
  }
  document.documentElement.dataset.theme = preference;

  document.addEventListener("DOMContentLoaded", () => {
    const select = document.getElementById("theme");
    select.value = preference;
    select.addEventListener("change", () => {
      preference = select.value;
      document.documentElement.dataset.theme = preference;
      try {
        localStorage.setItem(key, preference);
      } catch (error) {
        if (!["SecurityError", "QuotaExceededError"].includes(error.name)) throw error;
        console.warn("Loop applied your theme but could not save it for your next visit.", error);
      }
    });
  });
})();
