import type { NextPage } from 'next'
import Head from 'next/head'
import Image from 'next/image'
import Link from 'next/link'
import { Header } from '../components/Header'
import { HeaderLink } from '../components/HeaderLink'
import { SearchBar } from '../components/SearchBar'
import styles from '../styles/index.module.css'

const Home: NextPage = () => {
  return (
    <div className={styles.container}>
      <Head>
        <title>Blockscout Multi Search</title>
        <link rel="icon" href="/favicon.ico"/>
      </Head>

      <main className={styles.main}>
        <Header/>
        <div className={styles.content}>
          <SearchBar></SearchBar>
          
        </div>
      </main>
    </div>
  )
}

export default Home
