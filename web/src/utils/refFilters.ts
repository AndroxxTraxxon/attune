export function matchesRefFilter(
  actualRef: string | null | undefined,
  filterRef: string | undefined,
): boolean {
  if (!filterRef) return true;
  if (!actualRef) return false;

  if (filterRef.includes("*")) {
    return matchesWildcardRef(actualRef, filterRef);
  }

  return actualRef === filterRef;
}

function matchesWildcardRef(actualRef: string, filterRef: string): boolean {
  let refIndex = 0;
  let filterIndex = 0;
  let lastStarIndex = -1;
  let refIndexAfterLastStar = 0;

  while (refIndex < actualRef.length) {
    if (
      filterIndex < filterRef.length &&
      filterRef[filterIndex] === actualRef[refIndex]
    ) {
      refIndex += 1;
      filterIndex += 1;
    } else if (
      filterIndex < filterRef.length &&
      filterRef[filterIndex] === "*"
    ) {
      lastStarIndex = filterIndex;
      refIndexAfterLastStar = refIndex;
      filterIndex += 1;
    } else if (lastStarIndex !== -1) {
      filterIndex = lastStarIndex + 1;
      refIndexAfterLastStar += 1;
      refIndex = refIndexAfterLastStar;
    } else {
      return false;
    }
  }

  while (filterIndex < filterRef.length && filterRef[filterIndex] === "*") {
    filterIndex += 1;
  }

  return filterIndex === filterRef.length;
}

export function packPrefix(ref: string | null | undefined): string | undefined {
  if (!ref) return undefined;
  const [pack] = ref.split(".");
  return pack || undefined;
}
