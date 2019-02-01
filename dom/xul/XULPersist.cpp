/* -*- Mode: C++; tab-width: 8; indent-tabs-mode: nil; c-basic-offset: 2 -*- */
/* vim: set ts=8 sts=2 et sw=2 tw=80: */
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#include "XULPersist.h"
#include "XULStore.h"

namespace mozilla {
namespace dom {

static bool ShouldPersistAttribute(Element* aElement, nsAtom* aAttribute) {
  if (aElement->IsXULElement(nsGkAtoms::window)) {
    // This is not an element of the top document, its owner is
    // not an nsXULWindow. Persist it.
    if (aElement->OwnerDoc()->GetParentDocument()) {
      return true;
    }
    // The following attributes of xul:window should be handled in
    // nsXULWindow::SavePersistentAttributes instead of here.
    if (aAttribute == nsGkAtoms::screenX || aAttribute == nsGkAtoms::screenY ||
        aAttribute == nsGkAtoms::width || aAttribute == nsGkAtoms::height ||
        aAttribute == nsGkAtoms::sizemode) {
      return false;
    }
  }
  return true;
}

NS_IMPL_ISUPPORTS(XULPersist, nsIDocumentObserver)

XULPersist::XULPersist(Document* aDocument)
    : nsStubDocumentObserver(), mDocument(aDocument) {}

XULPersist::~XULPersist() {}

void XULPersist::Init() {
  ApplyPersistentAttributes();
  mDocument->AddObserver(this);
}

void XULPersist::DropDocumentReference() {
  mDocument->RemoveObserver(this);
  mDocument = nullptr;
}

void XULPersist::AttributeChanged(dom::Element* aElement, int32_t aNameSpaceID,
                                  nsAtom* aAttribute, int32_t aModType,
                                  const nsAttrValue* aOldValue) {
  NS_ASSERTION(aElement->OwnerDoc() == mDocument, "unexpected doc");

  // Might not need this, but be safe for now.
  nsCOMPtr<nsIDocumentObserver> kungFuDeathGrip(this);

  // See if there is anything we need to persist in the localstore.
  //
  // XXX Namespace handling broken :-(
  nsAutoString persist;
  aElement->GetAttr(kNameSpaceID_None, nsGkAtoms::persist, persist);
  // Persistence of attributes of xul:window is handled in nsXULWindow.
  if (ShouldPersistAttribute(aElement, aAttribute) && !persist.IsEmpty() &&
      // XXXldb This should check that it's a token, not just a substring.
      persist.Find(nsDependentAtomString(aAttribute)) >= 0) {
    nsContentUtils::AddScriptRunner(
        NewRunnableMethod<Element*, int32_t, nsAtom*>(
            "dom::XULPersist::Persist", this, &XULPersist::Persist, aElement,
            kNameSpaceID_None, aAttribute));
  }
}

void XULPersist::Persist(Element* aElement, int32_t aNameSpaceID,
                         nsAtom* aAttribute) {
  if (!mDocument) {
    return;
  }
  // For non-chrome documents, persistance is simply broken
  if (!nsContentUtils::IsSystemPrincipal(mDocument->NodePrincipal())) {
    return;
  }

  nsAutoString id;

  aElement->GetAttr(kNameSpaceID_None, nsGkAtoms::id, id);
  nsAtomString attrstr(aAttribute);

  nsAutoString valuestr;
  aElement->GetAttr(kNameSpaceID_None, aAttribute, valuestr);

  nsAutoCString utf8uri;
  nsresult rv = mDocument->GetDocumentURI()->GetSpec(utf8uri);
  if (NS_WARN_IF(NS_FAILED(rv))) {
    return;
  }
  NS_ConvertUTF8toUTF16 uri(utf8uri);

  bool hasAttr = xulstore_has_value(&uri, &id, &attrstr);

  if (hasAttr && valuestr.IsEmpty()) {
    Unused << xulstore_remove_value(&uri, &id, &attrstr);
    return;
  }

  // Persisting attributes to top level windows is handled by nsXULWindow.
  if (aElement->IsXULElement(nsGkAtoms::window)) {
    if (nsCOMPtr<nsIXULWindow> win =
            mDocument->GetXULWindowIfToplevelChrome()) {
      return;
    }
  }

  Unused << xulstore_set_value(&uri, &id, &attrstr, &valuestr);
}

nsresult XULPersist::ApplyPersistentAttributes() {
  if (!mDocument) {
    return NS_ERROR_NOT_AVAILABLE;
  }
  // For non-chrome documents, persistance is simply broken
  if (!nsContentUtils::IsSystemPrincipal(mDocument->NodePrincipal())) {
    return NS_ERROR_NOT_AVAILABLE;
  }

  // Add all of the 'persisted' attributes into the content
  // model.
  ApplyPersistentAttributesInternal();

  return NS_OK;
}

nsresult XULPersist::ApplyPersistentAttributesInternal() {
  nsCOMArray<Element> elements;

  nsAutoCString utf8uri;
  nsresult rv = mDocument->GetDocumentURI()->GetSpec(utf8uri);
  if (NS_WARN_IF(NS_FAILED(rv))) {
    return rv;
  }
  NS_ConvertUTF8toUTF16 uri(utf8uri);

  // Get a list of element IDs for which persisted values are available
  void* ids = xulstore_get_ids_iterator(&uri);

  while (xulstore_iter_has_more(ids)) {
    nsAutoString id;
    Unused << xulstore_iter_get_next(ids, &id);

    // We want to hold strong refs to the elements while applying
    // persistent attributes, just in case.
    const nsTArray<Element*>* allElements = mDocument->GetAllElementsForId(id);
    if (!allElements) {
      continue;
    }
    elements.Clear();
    elements.SetCapacity(allElements->Length());
    for (Element* element : *allElements) {
      elements.AppendObject(element);
    }

    rv = ApplyPersistentAttributesToElements(id, elements);
    if (NS_WARN_IF(NS_FAILED(rv))) {
      return rv;
    }
  }
  xulstore_iter_drop(ids);

  return NS_OK;
}

nsresult XULPersist::ApplyPersistentAttributesToElements(
    const nsAString& aID, nsCOMArray<Element>& aElements) {
  nsAutoCString utf8uri;
  nsresult rv = mDocument->GetDocumentURI()->GetSpec(utf8uri);
  if (NS_WARN_IF(NS_FAILED(rv))) {
    return rv;
  }
  NS_ConvertUTF8toUTF16 uri(utf8uri);

  // Get a list of attributes for which persisted values are available
  void* attrs = xulstore_get_attribute_iterator(&uri, &aID);

  while (xulstore_iter_has_more(attrs)) {
    nsAutoString attrstr;
    Unused << xulstore_iter_get_next(attrs, &attrstr);

    nsAutoString value;
    rv = xulstore_get_value(&uri, &aID, &attrstr, &value);
    if (NS_WARN_IF(NS_FAILED(rv))) {
      return rv;
    }

    RefPtr<nsAtom> attr = NS_Atomize(attrstr);
    if (NS_WARN_IF(!attr)) {
      return NS_ERROR_OUT_OF_MEMORY;
    }

    uint32_t cnt = aElements.Length();
    for (int32_t i = int32_t(cnt) - 1; i >= 0; --i) {
      Element* element = aElements.SafeElementAt(i);
      if (!element) {
        continue;
      }

      // Applying persistent attributes to top level windows is handled
      // by nsXULWindow.
      if (element->IsXULElement(nsGkAtoms::window)) {
        if (nsCOMPtr<nsIXULWindow> win =
                mDocument->GetXULWindowIfToplevelChrome()) {
          continue;
        }
      }

      Unused << element->SetAttr(kNameSpaceID_None, attr, value, true);
    }
  }
  xulstore_iter_drop(attrs);

  return NS_OK;
}

}  // namespace dom
}  // namespace mozilla
