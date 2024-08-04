async function getRecentMemes() {
  try {
    // Make the network request
    const response = await fetch("/meme");

    // Check if the response is ok (status code 200-299)
    if (!response.ok) {
      throw new Error(`HTTP error! Status: ${response.status}`);
    }

    // Parse the JSON data from the response
    const data = await response.json();

    // Handle the data (e.g., display it in the console)
    return data;

    // You can also manipulate the DOM or do other things with the data here
    // Example: document.getElementById('output').textContent = JSON.stringify(data, null, 2);
  } catch (error) {
    // Handle errors (e.g., network issues, parsing errors)
    console.error("Fetch error:", error);
  }
}
