const pad2 = (value: number) => value.toString().padStart(2, "0");

export const formatDateTime = (timestampMs: number): string => {
    const date = new Date(timestampMs);
    if (Number.isNaN(date.getTime())) return "Invalid date";
    const year = date.getFullYear();
    const month = pad2(date.getMonth() + 1);
    const day = pad2(date.getDate());
    const hours = pad2(date.getHours());
    const minutes = pad2(date.getMinutes());
    return `${year}-${month}-${day} ${hours}:${minutes}`;
};
