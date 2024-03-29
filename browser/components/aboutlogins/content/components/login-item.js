/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

import {recordTelemetryEvent} from "../aboutLoginsUtils.js";
import ReflectedFluentElement from "./reflected-fluent-element.js";

export default class LoginItem extends ReflectedFluentElement {
  constructor() {
    super();
    this._login = {};
  }

  connectedCallback() {
    if (this.shadowRoot) {
      this.render();
      return;
    }

    let loginItemTemplate = document.querySelector("#login-item-template");
    this.attachShadow({mode: "open"})
        .appendChild(loginItemTemplate.content.cloneNode(true));

    this.reflectFluentStrings();

    for (let selector of [
      ".delete-button",
      ".edit-button",
      ".open-site-button",
      ".save-changes-button",
      ".cancel-button",
    ]) {
      let button = this.shadowRoot.querySelector(selector);
      button.addEventListener("click", this);
    }

    window.addEventListener("AboutLoginsLoginSelected", this);

    let originInput = this.shadowRoot.querySelector("input[name='origin']");
    originInput.addEventListener("blur", this);

    let revealCheckbox = this.shadowRoot.querySelector(".reveal-password-checkbox");
    revealCheckbox.addEventListener("click", this);

    let copyUsernameButton = this.shadowRoot.querySelector(".copy-username-button");
    let copyPasswordButton = this.shadowRoot.querySelector(".copy-password-button");
    copyUsernameButton.relatedInput = this.shadowRoot.querySelector("input[name='username']");
    copyPasswordButton.relatedInput = this.shadowRoot.querySelector("input[name='password']");

    this.render();
  }

  static get reflectedFluentIDs() {
    return [
      "cancel-button",
      "copied-password-button",
      "copied-username-button",
      "copy-password-button",
      "copy-username-button",
      "delete-button",
      "edit-button",
      "new-login-title",
      "open-site-button",
      "origin-label",
      "origin-placeholder",
      "password-hide-title",
      "password-label",
      "password-show-title",
      "save-changes-button",
      "time-created",
      "time-changed",
      "time-used",
      "username-label",
      "username-placeholder",
    ];
  }

  static get observedAttributes() {
    return this.reflectedFluentIDs;
  }

  handleSpecialCaseFluentString(attrName) {
    switch (attrName) {
      case "copied-password-button":
      case "copy-password-button": {
        let copyPasswordButton = this.shadowRoot.querySelector(".copy-password-button");
        let newAttrName = attrName.substr(0, attrName.indexOf("-")) + "-button-text";
        copyPasswordButton.setAttribute(newAttrName, this.getAttribute(attrName));
        break;
      }
      case "copied-username-button":
      case "copy-username-button": {
        let copyUsernameButton = this.shadowRoot.querySelector(".copy-username-button");
        let newAttrName = attrName.substr(0, attrName.indexOf("-")) + "-button-text";
        copyUsernameButton.setAttribute(newAttrName, this.getAttribute(attrName));
        break;
      }
      case "new-login-title": {
        let title = this.shadowRoot.querySelector(".title");
        title.setAttribute(attrName, this.getAttribute(attrName));
        if (!this._login.title) {
          title.textContent = this.getAttribute(attrName);
        }
        break;
      }
      case "origin-placeholder": {
        let originInput = this.shadowRoot.querySelector("input[name='origin']");
        originInput.setAttribute("placeholder", this.getAttribute(attrName));
        break;
      }
      case "password-hide-title":
      case "password-show-title": {
        this.updatePasswordRevealState();
        break;
      }
      case "username-placeholder": {
        let usernameInput = this.shadowRoot.querySelector("input[name='username']");
        usernameInput.setAttribute("placeholder", this.getAttribute(attrName));
        break;
      }
      default:
        return false;
    }
    return true;
  }

  render() {
    let l10nArgs = {
      timeCreated: this._login.timeCreated || "",
      timeChanged: this._login.timePasswordChanged || "",
      timeUsed: this._login.timeLastUsed || "",
    };
    document.l10n.setAttributes(this, "login-item", l10nArgs);

    let title = this.shadowRoot.querySelector(".title");
    title.textContent = this._login.title || title.getAttribute("new-login-title");
    this.shadowRoot.querySelector("input[name='origin']").defaultValue = this._login.origin || "";
    this.shadowRoot.querySelector("input[name='username']").defaultValue = this._login.username || "";
    this.shadowRoot.querySelector("input[name='password']").defaultValue = this._login.password || "";
    this.updatePasswordRevealState();
  }

  handleEvent(event) {
    switch (event.type) {
      case "AboutLoginsLoginSelected": {
        this.setLogin(event.detail);
        break;
      }
      case "blur": {
        // Add https:// prefix if one was not provided.
        let originInput = this.shadowRoot.querySelector("input[name='origin']");
        let originValue = originInput.value.trim();
        if (!originValue) {
          return;
        }
        if (!originValue.match(/:\/\//)) {
          originInput.value = "https://" + originValue;
        }
        break;
      }
      case "click": {
        if (event.target.classList.contains("cancel-button")) {
          if (this._login.guid) {
            this.setLogin(this._login);
          } else {
            // TODO, should select the first login if it exists
            // or show the no-logins view otherwise
            this.toggleEditing();
            this.render();
          }

          recordTelemetryEvent({
            object: this._login.guid ? "existing_login" : "new_login",
            method: "cancel",
          });
          return;
        }
        if (event.target.classList.contains("delete-button")) {
          document.dispatchEvent(new CustomEvent("AboutLoginsDeleteLogin", {
            bubbles: true,
            detail: this._login,
          }));

          recordTelemetryEvent({object: "existing_login", method: "delete"});
          return;
        }
        if (event.target.classList.contains("edit-button")) {
          this.toggleEditing();

          recordTelemetryEvent({object: "existing_login", method: "edit"});
          return;
        }
        if (event.target.classList.contains("open-site-button")) {
          document.dispatchEvent(new CustomEvent("AboutLoginsOpenSite", {
            bubbles: true,
            detail: this._login,
          }));

          recordTelemetryEvent({object: "existing_login", method: "open_site"});
          return;
        }
        if (event.target.classList.contains("reveal-password-checkbox")) {
          this.updatePasswordRevealState();

          let revealCheckbox = this.shadowRoot.querySelector(".reveal-password-checkbox");
          let method = revealCheckbox.checked ? "show" : "hide";
          recordTelemetryEvent({object: "password", method});
          return;
        }
        if (event.target.classList.contains("save-changes-button")) {
          if (!this._isFormValid({reportErrors: true})) {
            return;
          }
          let loginUpdates = this._loginFromForm();
          if (this._login.guid) {
            loginUpdates.guid = this._login.guid;
            document.dispatchEvent(new CustomEvent("AboutLoginsUpdateLogin", {
              bubbles: true,
              detail: loginUpdates,
            }));

            recordTelemetryEvent({object: "existing_login", method: "save"});
          } else {
            document.dispatchEvent(new CustomEvent("AboutLoginsCreateLogin", {
              bubbles: true,
              detail: loginUpdates,
            }));

            recordTelemetryEvent({object: "new_login", method: "save"});
          }
        }
        break;
      }
    }
  }

  setLogin(login) {
    this._login = login;

    let originInput =
      this.resetValidation(this.shadowRoot.querySelector("input[name='origin']"), login.origin);
    originInput.addEventListener("blur", this);
    let usernameInput =
      this.resetValidation(this.shadowRoot.querySelector("input[name='username']"), login.username);
    let passwordInput =
      this.resetValidation(this.shadowRoot.querySelector("input[name='password']"), login.password);

    let copyUsernameButton = this.shadowRoot.querySelector(".copy-username-button");
    let copyPasswordButton = this.shadowRoot.querySelector(".copy-password-button");
    copyUsernameButton.relatedInput = usernameInput;
    copyPasswordButton.relatedInput = passwordInput;

    this.toggleAttribute("isNewLogin", !login.guid);
    this.toggleEditing(!login.guid);

    let revealCheckbox = this.shadowRoot.querySelector(".reveal-password-checkbox");
    revealCheckbox.checked = false;

    this.render();
  }

  loginAdded(login) {
    if (this._login.guid ||
        !window.AboutLoginsUtils.doLoginsMatch(login, this._loginFromForm())) {
      return;
    }

    this.toggleEditing(false);
    this._login = login;
    this.render();
  }

  loginModified(login) {
    if (this._login.guid != login.guid) {
      return;
    }

    this.toggleEditing(false);
    this._login = login;
    this.render();
  }

  loginRemoved(login) {
    if (login.guid != this._login.guid) {
      return;
    }

    this.toggleEditing(false);
    this._login = {};
    this.render();
  }

  toggleEditing(force) {
    let shouldEdit = force !== undefined ? force : !this.hasAttribute("editing");

    if (!shouldEdit) {
      this.removeAttribute("isNewLogin");
    }

    if (shouldEdit) {
      this.shadowRoot.querySelector("input[name='password']").style.removeProperty("width");
    } else {
      // Need to set a shorter width than -moz-available so the reveal checkbox
      // will still appear next to the password.
      this.shadowRoot.querySelector("input[name='password']").style.width =
        (this._login.password || "").length + "ch";
    }

    this.shadowRoot.querySelector(".delete-button").disabled = this.hasAttribute("isNewLogin");
    this.shadowRoot.querySelector(".edit-button").disabled = shouldEdit;
    this.shadowRoot.querySelector("input[name='origin']").readOnly = !this.hasAttribute("isNewLogin");
    this.shadowRoot.querySelector("input[name='username']").readOnly = !shouldEdit;
    this.shadowRoot.querySelector("input[name='password']").readOnly = !shouldEdit;
    this.toggleAttribute("editing", shouldEdit);
  }

  resetValidation(formElement, value) {
    let wasRequired = formElement.required;
    let newFormElement = document.createElement(formElement.localName);
    if (value) {
      newFormElement.defaultValue = value;
    }
    newFormElement.className = formElement.className;
    newFormElement.placeholder = formElement.placeholder;
    newFormElement.setAttribute("name", formElement.getAttribute("name"));
    newFormElement.setAttribute("type", formElement.getAttribute("type"));
    if (wasRequired) {
      newFormElement.required = true;
    }
    formElement.replaceWith(newFormElement);
    return newFormElement;
  }

  updatePasswordRevealState() {
    let revealCheckbox = this.shadowRoot.querySelector(".reveal-password-checkbox");
    let labelAttr = revealCheckbox.checked ? "password-show-title"
                                           : "password-hide-title";
    revealCheckbox.setAttribute("aria-label", this.getAttribute(labelAttr));
    revealCheckbox.setAttribute("title", this.getAttribute(labelAttr));

    let passwordInput = this.shadowRoot.querySelector("input[name='password']");
    if (revealCheckbox.checked) {
      passwordInput.setAttribute("type", "text");
      return;
    }
    passwordInput.setAttribute("type", "password");
  }

  /**
   * Checks that the edit/new-login form has valid values present for their
   * respective required fields.
   *
   * @param {boolean} reportErrors If true, validation errors will be reported
   *                               to the user.
   */
  _isFormValid({reportErrors} = {}) {
    let fields = [this.shadowRoot.querySelector("input[name='password']")];
    if (this.hasAttribute("isNewLogin")) {
      fields.push(this.shadowRoot.querySelector("input[name='origin']"));
    }
    let valid = true;
    // Check validity on all required fields so each field will get :invalid styling
    // if applicable.
    for (let field of fields) {
      if (reportErrors) {
        valid &= field.reportValidity();
      } else {
        valid &= field.checkValidity();
      }
    }
    return valid;
  }

  _loginFromForm() {
    return {
      username: this.shadowRoot.querySelector("input[name='username']").value.trim(),
      password: this.shadowRoot.querySelector("input[name='password']").value.trim(),
      origin: this.shadowRoot.querySelector("input[name='origin']").value.trim(),
    };
  }
}
customElements.define("login-item", LoginItem);
