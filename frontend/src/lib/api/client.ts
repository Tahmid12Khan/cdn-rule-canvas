import { z } from "zod";

export const API_BASE =
  process.env.NEXT_PUBLIC_API_BASE ?? "http://localhost:8000";

// Error envelope (BACKEND CONTRACT §1)
export const ApiErrorDetail = z.object({
  loc: z.string(),
  msg: z.string(),
  rule_id: z.string(),
});
export type ApiErrorDetail = z.infer<typeof ApiErrorDetail>;

export const ApiErrorBody = z.object({
  error: z.object({
    code: z.string(),
    message: z.string(),
    details: z.array(ApiErrorDetail).nullish(),
  }),
});
export type ApiErrorBody = z.infer<typeof ApiErrorBody>;

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
    public details?: ApiErrorDetail[],
    // The raw, unparsed response body text (req 3). Surfaced verbatim in the
    // ErrorBanner "Show full server response" accordion. Captured once via
    // res.text() so we never double-read the body.
    public rawBody?: string,
  ) {
    super(message);
    this.name = "ApiError";
  }

  get isValidation(): boolean {
    return this.status === 422 || this.code === "VALIDATION_ERROR";
  }
}

// Parse the error envelope out of an already-read body string (no second
// res.json() — a Response body can only be read once). Shared by client.ts and
// evalTest.ts so both attach the raw body to ApiError.
export function parseApiError(
  status: number,
  statusText: string,
  rawBody: string,
): ApiError {
  let json: unknown = null;
  try {
    json = JSON.parse(rawBody);
  } catch {
    json = null;
  }
  const parsed = ApiErrorBody.safeParse(json);
  const e = parsed.success
    ? parsed.data.error
    : { code: "INTERNAL_ERROR", message: statusText, details: undefined };
  return new ApiError(
    status,
    e.code,
    e.message,
    e.details ?? undefined,
    rawBody,
  );
}

// Pagination envelope (BACKEND CONTRACT §5)
export const Page = <T extends z.ZodTypeAny>(item: T) =>
  z.object({
    items: z.array(item),
    page: z.number(),
    page_size: z.number(),
    total: z.number(),
  });

async function request<T>(
  path: string,
  schema: z.ZodType<T>,
  init?: RequestInit,
): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    ...init,
    headers: { "Content-Type": "application/json", ...(init?.headers ?? {}) },
  });

  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw parseApiError(res.status, res.statusText, text);
  }

  if (res.status === 204) {
    return undefined as T;
  }

  return schema.parse(await res.json());
}

export const apiGet = <T>(path: string, schema: z.ZodType<T>): Promise<T> =>
  request(path, schema);

export const apiSend = <T>(
  method: string,
  path: string,
  schema: z.ZodType<T>,
  body?: unknown,
): Promise<T> =>
  request(path, schema, {
    method,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
