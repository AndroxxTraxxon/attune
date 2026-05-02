from http import HTTPStatus
from typing import Any, cast
from urllib.parse import quote

import httpx

from ...client import AuthenticatedClient, Client
from ...types import Response, UNSET
from ... import errors

from ...models.paginated_response_work_queue_summary import PaginatedResponseWorkQueueSummary
from ...types import UNSET, Unset
from typing import cast



def _get_kwargs(
    *,
    enabled: bool | None | Unset = UNSET,
    is_adhoc: bool | None | Unset = UNSET,
    search: None | str | Unset = UNSET,
    page: int | Unset = UNSET,
    per_page: int | Unset = UNSET,

) -> dict[str, Any]:
    

    

    params: dict[str, Any] = {}

    json_enabled: bool | None | Unset
    if isinstance(enabled, Unset):
        json_enabled = UNSET
    else:
        json_enabled = enabled
    params["enabled"] = json_enabled

    json_is_adhoc: bool | None | Unset
    if isinstance(is_adhoc, Unset):
        json_is_adhoc = UNSET
    else:
        json_is_adhoc = is_adhoc
    params["is_adhoc"] = json_is_adhoc

    json_search: None | str | Unset
    if isinstance(search, Unset):
        json_search = UNSET
    else:
        json_search = search
    params["search"] = json_search

    params["page"] = page

    params["per_page"] = per_page


    params = {k: v for k, v in params.items() if v is not UNSET and v is not None}


    _kwargs: dict[str, Any] = {
        "method": "get",
        "url": "/api/v1/queues",
        "params": params,
    }


    return _kwargs



def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> PaginatedResponseWorkQueueSummary | None:
    if response.status_code == 200:
        response_200 = PaginatedResponseWorkQueueSummary.from_dict(response.json())



        return response_200

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Response[PaginatedResponseWorkQueueSummary]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient,
    enabled: bool | None | Unset = UNSET,
    is_adhoc: bool | None | Unset = UNSET,
    search: None | str | Unset = UNSET,
    page: int | Unset = UNSET,
    per_page: int | Unset = UNSET,

) -> Response[PaginatedResponseWorkQueueSummary]:
    """ 
    Args:
        enabled (bool | None | Unset):
        is_adhoc (bool | None | Unset):
        search (None | str | Unset):
        page (int | Unset):
        per_page (int | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[PaginatedResponseWorkQueueSummary]
     """


    kwargs = _get_kwargs(
        enabled=enabled,
is_adhoc=is_adhoc,
search=search,
page=page,
per_page=per_page,

    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)

def sync(
    *,
    client: AuthenticatedClient,
    enabled: bool | None | Unset = UNSET,
    is_adhoc: bool | None | Unset = UNSET,
    search: None | str | Unset = UNSET,
    page: int | Unset = UNSET,
    per_page: int | Unset = UNSET,

) -> PaginatedResponseWorkQueueSummary | None:
    """ 
    Args:
        enabled (bool | None | Unset):
        is_adhoc (bool | None | Unset):
        search (None | str | Unset):
        page (int | Unset):
        per_page (int | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        PaginatedResponseWorkQueueSummary
     """


    return sync_detailed(
        client=client,
enabled=enabled,
is_adhoc=is_adhoc,
search=search,
page=page,
per_page=per_page,

    ).parsed

async def asyncio_detailed(
    *,
    client: AuthenticatedClient,
    enabled: bool | None | Unset = UNSET,
    is_adhoc: bool | None | Unset = UNSET,
    search: None | str | Unset = UNSET,
    page: int | Unset = UNSET,
    per_page: int | Unset = UNSET,

) -> Response[PaginatedResponseWorkQueueSummary]:
    """ 
    Args:
        enabled (bool | None | Unset):
        is_adhoc (bool | None | Unset):
        search (None | str | Unset):
        page (int | Unset):
        per_page (int | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[PaginatedResponseWorkQueueSummary]
     """


    kwargs = _get_kwargs(
        enabled=enabled,
is_adhoc=is_adhoc,
search=search,
page=page,
per_page=per_page,

    )

    response = await client.get_async_httpx_client().request(
        **kwargs
    )

    return _build_response(client=client, response=response)

async def asyncio(
    *,
    client: AuthenticatedClient,
    enabled: bool | None | Unset = UNSET,
    is_adhoc: bool | None | Unset = UNSET,
    search: None | str | Unset = UNSET,
    page: int | Unset = UNSET,
    per_page: int | Unset = UNSET,

) -> PaginatedResponseWorkQueueSummary | None:
    """ 
    Args:
        enabled (bool | None | Unset):
        is_adhoc (bool | None | Unset):
        search (None | str | Unset):
        page (int | Unset):
        per_page (int | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        PaginatedResponseWorkQueueSummary
     """


    return (await asyncio_detailed(
        client=client,
enabled=enabled,
is_adhoc=is_adhoc,
search=search,
page=page,
per_page=per_page,

    )).parsed
