async function main() {
  await new Promise((resolve) => setTimeout(resolve, 250));
  await new Promise((resolve) => setTimeout(resolve, 250));
  console.log(JSON.stringify({ success: true, delaysCompleted: 2 }));
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
